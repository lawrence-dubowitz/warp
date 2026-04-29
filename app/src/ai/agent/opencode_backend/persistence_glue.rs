//! Read/write helpers for the new opencode-specific fields on
//! `AgentConversationData`.
//!
//! `specs/opencode-as-primary-agent/TECH.md` §8 specifies three new
//! JSON fields — no schema migration since
//! `AgentConversationData` is a JSON blob:
//!
//! - `backend_kind: Option<BackendKind>` — locked at conversation
//!   creation time; the feature-flag flip never moves existing
//!   conversations between backends.
//! - `opencode_session_id: Option<String>` — created lazily on first
//!   prompt, reused across Warp restarts.
//! - `opencode_id_map: Option<OpencodeIdMap>` — maps opencode wire ids
//!   (`message_id`, `part_id`, `call_id`) to the Warp ids
//!   (`AIAgentExchangeId`, `AIAgentMessageId`, `AIAgentActionId`) used
//!   internally. Deduping SSE-vs-RPC tool results uses the call-id
//!   half of this.
//!
//! `server_conversation_token` is **never reused** for opencode — the
//! token stays Warp-cloud only.
//!
//! Phase 1: stubs. The fields themselves and their `serde` glue are
//! added in Stream 3 (`bd: warp-317.3`).
//!
//! Spec: `specs/opencode-as-primary-agent/TECH.md` §8.
//!
//! bd: warp-317.1.6 (skeleton); warp-317.3 (real fields + helpers).

/// Identifies which backend a conversation runs on.
///
/// Persisted on `AgentConversationData` once Stream 3 lands; pinned at
/// conversation creation time so flag flips never migrate existing
/// conversations.
#[allow(dead_code)] // Persistence wiring lands in warp-317.3.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BackendKind {
    /// The legacy multi-agent Warp backend.
    Warp,
    /// The opencode-as-primary-agent backend (this module).
    Opencode,
}
