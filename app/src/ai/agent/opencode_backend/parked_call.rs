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
//! Phase 1: types only. Methods are `unimplemented!()` and are wired in
//! Stream 2 (`bd: warp-317.2`).
//!
//! Spec: `specs/opencode-as-primary-agent/TECH.md` §3, §9.
//!
//! bd: warp-317.1.6 (skeleton); warp-317.2 (state-machine + executor
//! event subscriptions); warp-317.3 (cancellation/abort plumbing).

/// State of a parked opencode tool call.
///
/// Variants follow the diagram above in source order.
#[allow(dead_code)] // Variants consumed once warp-317.2 wires the machine.
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

/// A single tool invocation parked between opencode (the requester)
/// and Warp's executor (the runner).
///
/// Real fields land in `bd: warp-317.2`: opencode `call_id`, original
/// tool name, input payload, deserialised `AIAgentAction`, current
/// state, and the `oneshot::Sender` that delivers the result back to
/// the RPC handler.
#[allow(dead_code)] // Real fields added in warp-317.2.
pub(crate) struct ParkedToolCall;
