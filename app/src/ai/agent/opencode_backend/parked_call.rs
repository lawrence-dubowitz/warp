//! Per-tool-call state machine for opencode tool invocations.
//!
//! When the opencode plugin asks Warp to run a tool, Warp parks the
//! call and walks it through the same preprocess/approval/execute
//! pipeline the legacy backend uses, then returns the result via JSON-RPC.
//! This means opencode's view of "tool X ran and produced Y" is
//! identical to "Warp's existing block lifecycle ran".
//!
//! The state machine, copied from
//! `specs/opencode-as-primary-agent/TECH.md` §3:
//!
//! ```text
//! Submitted
//!   → AwaitingPreprocess
//!   → AwaitingApproval
//!   → Approved
//!   → Executing
//!   → { Completed | Cancelled | Failed | SubprocessLost }
//! ```
//!
//! - `Completed` / `Failed` come from the regular block-execution
//!   pipeline (Warp's `BlocklistAIActionExecutorEvent` stream).
//! - `Cancelled` is reached when the user aborts the turn (Warp issues
//!   `POST /session/:id/abort` to opencode and cancels every parked
//!   call in flight).
//! - `SubprocessLost` is reached when the supervisor restarts opencode
//!   mid-turn — every parked call transitions there and the user sees
//!   "opencode restarted; this turn was interrupted. Try again."
//!
//! ## Dedup invariant
//!
//! opencode's plugin shadows ALL built-in tools. If a tool result
//! arrives via SSE (meaning the shadow leaked and opencode ran the
//! tool itself), the `ParkedCallRegistry` detects the duplicate by
//! `opencode_call_id` and emits `tracing::warn`. The RPC-side result
//! is authoritative; the SSE result is dropped.
//!
//! Spec: `specs/opencode-as-primary-agent/TECH.md` §3, §9.
//!
//! bd: warp-317.1.6 (skeleton); warp-317.2.5 (state machine + dedup).

use std::collections::HashMap;

use serde_json::Value;
use tokio::sync::oneshot;
use tracing::warn;

use crate::ai::agent::AIAgentActionId;

// ---------------------------------------------------------------------------
// State enum
// ---------------------------------------------------------------------------

/// State of a parked opencode tool call.
///
/// Variants follow the state diagram above in source order.
/// Terminal states: `Completed`, `Cancelled`, `Failed`, `SubprocessLost`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ParkedToolCallState {
    /// Plugin RPC received; not yet handed to Warp's preprocess pass.
    Submitted,
    /// Awaiting Warp's per-action preprocess hooks (input shaping,
    /// path canonicalisation, etc.).
    AwaitingPreprocess,
    /// Awaiting user/auto approval.
    AwaitingApproval,
    /// Approved; queued behind any earlier in-flight actions.
    Approved,
    /// Currently executing in the Warp block pipeline.
    Executing,
    /// Terminal: ran to completion, RPC response sent.
    Completed,
    /// Terminal: user aborted the turn.
    Cancelled,
    /// Terminal: execution returned an error.
    Failed,
    /// Terminal: opencode subprocess restarted mid-flight.
    SubprocessLost,
}

impl ParkedToolCallState {
    #[allow(dead_code)] // Used by tests + future consumers (warp-317.2.4/317.2.6).
    pub(crate) fn is_terminal(self) -> bool {
        matches!(
            self,
            Self::Completed | Self::Cancelled | Self::Failed | Self::SubprocessLost
        )
    }
}

// ---------------------------------------------------------------------------
// Transition validation
// ---------------------------------------------------------------------------

/// Transition error — attempted a move between states that violates the
/// diagram.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct InvalidTransition {
    pub(crate) from: ParkedToolCallState,
    pub(crate) to: ParkedToolCallState,
}

impl std::fmt::Display for InvalidTransition {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "invalid ParkedToolCall transition: {:?} → {:?}",
            self.from, self.to
        )
    }
}

impl std::error::Error for InvalidTransition {}

/// Returns `true` if `from → to` is a valid edge in the state machine.
fn is_valid_transition(from: ParkedToolCallState, to: ParkedToolCallState) -> bool {
    use ParkedToolCallState::*;
    matches!(
        (from, to),
        (Submitted, AwaitingPreprocess)
            | (AwaitingPreprocess, AwaitingApproval)
            | (AwaitingApproval, Approved)
            | (Approved, Executing)
            | (Executing, Completed)
            | (Executing, Failed)
            // Cancellation can happen from any non-terminal state.
            | (Submitted, Cancelled)
            | (AwaitingPreprocess, Cancelled)
            | (AwaitingApproval, Cancelled)
            | (Approved, Cancelled)
            | (Executing, Cancelled)
            // SubprocessLost can happen from any non-terminal state.
            | (Submitted, SubprocessLost)
            | (AwaitingPreprocess, SubprocessLost)
            | (AwaitingApproval, SubprocessLost)
            | (Approved, SubprocessLost)
            | (Executing, SubprocessLost)
    )
}

// ---------------------------------------------------------------------------
// ParkedToolCall
// ---------------------------------------------------------------------------

/// The RPC result sent back to the opencode plugin.
#[derive(Debug, Clone)]
pub(crate) struct ToolResult {
    /// JSON-serialisable output payload. `None` means no output (cancelled
    /// or lost).
    pub(crate) output: Option<Value>,
    /// If the tool execution failed, a human-readable error message.
    pub(crate) error: Option<String>,
}

/// A single tool invocation parked between opencode (the requester)
/// and Warp's executor (the runner).
pub(crate) struct ParkedToolCall {
    /// opencode's call_id (opaque string, unique per-session).
    pub(crate) opencode_call_id: String,
    /// The tool name as opencode knows it (e.g. "bash", "read").
    pub(crate) tool_name: String,
    /// Raw input JSON from the plugin.
    pub(crate) input: Value,
    /// Warp-side action ID once queued (set after Submitted → AwaitingPreprocess).
    pub(crate) warp_action_id: Option<AIAgentActionId>,
    /// Current state.
    state: ParkedToolCallState,
    /// Channel back to the RPC handler. Consumed exactly once when
    /// entering a terminal state.
    response_tx: Option<oneshot::Sender<ToolResult>>,
}

impl ParkedToolCall {
    /// Create a new parked call in `Submitted` state.
    pub(crate) fn new(
        opencode_call_id: String,
        tool_name: String,
        input: Value,
        response_tx: oneshot::Sender<ToolResult>,
    ) -> Self {
        Self {
            opencode_call_id,
            tool_name,
            input,
            warp_action_id: None,
            state: ParkedToolCallState::Submitted,
            response_tx: Some(response_tx),
        }
    }

    /// Current state.
    pub(crate) fn state(&self) -> ParkedToolCallState {
        self.state
    }

    /// Attempt a state transition. Returns `Err(InvalidTransition)` if
    /// the edge is illegal.
    pub(crate) fn transition(&mut self, to: ParkedToolCallState) -> Result<(), InvalidTransition> {
        if !is_valid_transition(self.state, to) {
            return Err(InvalidTransition {
                from: self.state,
                to,
            });
        }
        self.state = to;
        Ok(())
    }

    /// Complete the call by sending the result to the waiting RPC handler.
    /// Transitions to `Completed` and consumes the response channel.
    ///
    /// If the call is already terminal (double-complete), returns
    /// `Err(InvalidTransition)`.
    pub(crate) fn complete(&mut self, result: ToolResult) -> Result<(), InvalidTransition> {
        self.transition(ParkedToolCallState::Completed)?;
        self.send_result(result);
        Ok(())
    }

    /// Mark the call as failed and notify the RPC handler.
    pub(crate) fn fail(&mut self, error: String) -> Result<(), InvalidTransition> {
        self.transition(ParkedToolCallState::Failed)?;
        self.send_result(ToolResult {
            output: None,
            error: Some(error),
        });
        Ok(())
    }

    /// Cancel the call (user abort). Notifies the RPC handler with
    /// a cancelled indicator.
    pub(crate) fn cancel(&mut self) -> Result<(), InvalidTransition> {
        self.transition(ParkedToolCallState::Cancelled)?;
        self.send_result(ToolResult {
            output: None,
            error: Some("cancelled".to_owned()),
        });
        Ok(())
    }

    /// Mark the call as lost due to subprocess restart.
    pub(crate) fn mark_subprocess_lost(&mut self) -> Result<(), InvalidTransition> {
        self.transition(ParkedToolCallState::SubprocessLost)?;
        self.send_result(ToolResult {
            output: None,
            error: Some("opencode restarted; this turn was interrupted. Try again.".to_owned()),
        });
        Ok(())
    }

    /// Send the result through the oneshot channel. Noop if the channel
    /// was already consumed (defensive against double-terminal).
    fn send_result(&mut self, result: ToolResult) {
        if let Some(tx) = self.response_tx.take() {
            // Receiver may have been dropped (e.g. request timeout);
            // that's fine — the result is just lost.
            let _ = tx.send(result);
        }
    }
}

// ---------------------------------------------------------------------------
// Registry (per-session collection of parked calls)
// ---------------------------------------------------------------------------

/// Per-session registry of in-flight parked tool calls.
///
/// Keyed by `opencode_call_id` for O(1) lookup from both the RPC
/// side (result delivery) and the SSE side (dedup detection).
pub(crate) struct ParkedCallRegistry {
    calls: HashMap<String, ParkedToolCall>,
}

impl ParkedCallRegistry {
    pub(crate) fn new() -> Self {
        Self {
            calls: HashMap::new(),
        }
    }

    /// Insert a new parked call. Panics if `opencode_call_id` is already
    /// present (double-submit is a protocol violation).
    pub(crate) fn insert(&mut self, call: ParkedToolCall) {
        let id = call.opencode_call_id.clone();
        if self.calls.insert(id.clone(), call).is_some() {
            panic!("duplicate opencode_call_id in ParkedCallRegistry: {id}");
        }
    }

    /// Look up a parked call by opencode_call_id.
    pub(crate) fn get_mut(&mut self, opencode_call_id: &str) -> Option<&mut ParkedToolCall> {
        self.calls.get_mut(opencode_call_id)
    }

    /// Remove a terminal call from the registry. Returns `None` if not
    /// found or not yet terminal.
    pub(crate) fn remove_if_terminal(&mut self, opencode_call_id: &str) -> Option<ParkedToolCall> {
        if self
            .calls
            .get(opencode_call_id)
            .is_some_and(|c| c.state().is_terminal())
        {
            self.calls.remove(opencode_call_id)
        } else {
            None
        }
    }

    /// SSE-side dedup check. If `opencode_call_id` is present in the
    /// registry (meaning we handled it via RPC), the SSE result is a
    /// duplicate — warn and return `true`. If absent, return `false`
    /// (meaning the call was NOT shadowed and SSE result is genuine —
    /// this should not happen in Phase 1, but is handled defensively).
    pub(crate) fn check_sse_dedup(&self, opencode_call_id: &str) -> bool {
        if self.calls.contains_key(opencode_call_id) {
            warn!(
                opencode_call_id,
                "SSE tool result arrived for RPC-handled call — plugin shadow leaked; \
                 dropping SSE result (RPC result is authoritative)"
            );
            true
        } else {
            false
        }
    }

    /// Cancel ALL non-terminal calls (user-abort scenario).
    /// Returns the count of calls that were successfully cancelled.
    pub(crate) fn cancel_all(&mut self) -> usize {
        let mut count = 0;
        for call in self.calls.values_mut() {
            if !call.state().is_terminal() {
                if call.cancel().is_ok() {
                    count += 1;
                }
            }
        }
        count
    }

    /// Mark ALL non-terminal calls as SubprocessLost.
    /// Returns the count of calls transitioned.
    pub(crate) fn mark_all_subprocess_lost(&mut self) -> usize {
        let mut count = 0;
        for call in self.calls.values_mut() {
            if !call.state().is_terminal() {
                if call.mark_subprocess_lost().is_ok() {
                    count += 1;
                }
            }
        }
        count
    }

    /// Number of calls currently in the registry (terminal + non-terminal).
    pub(crate) fn len(&self) -> usize {
        self.calls.len()
    }
}

#[cfg(test)]
#[path = "parked_call_tests.rs"]
mod parked_call_tests;
