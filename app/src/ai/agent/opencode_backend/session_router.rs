//! Maps Warp `conversation_id` ↔ opencode `session_id`.
//!
//! Each Warp conversation that runs on the opencode backend gets exactly
//! one opencode session, lazy-created on first prompt and reused across
//! Warp restarts (the id is persisted on the conversation; see
//! [`super::persistence_glue`]).
//!
//! The router is intentionally tiny in Phase 1: a thin wrapper that
//! turns "do I have a session for this conversation?" into either a
//! cached `String` or an `OpencodeClient::create_session` call. The
//! real impl lands in `bd: warp-317.2` (parallel to the RPC handlers
//! that need to address sessions).
//!
//! Spec: `specs/opencode-as-primary-agent/TECH.md` §1, §7, §8.
//!
//! bd: warp-317.1.6 (skeleton); warp-317.2 (real lookup + lazy create).

/// Lazy `conversation_id` → `opencode_session_id` resolver.
///
/// Currently a placeholder. The fielded version will hold either a
/// reference to the persistence layer (so it can read/write
/// `AgentConversationData::opencode_session_id`) or an in-memory cache
/// keyed by Warp conversation id.
#[allow(dead_code)] // Real fields added in warp-317.2.
pub(crate) struct SessionRouter;

impl SessionRouter {
    /// Resolve the opencode session for a Warp conversation, creating
    /// it on first call. Real impl lands in `bd: warp-317.2`.
    #[allow(dead_code)]
    pub(crate) async fn resolve(&self, _conversation_id: &str) -> String {
        unimplemented!("session_router::resolve — bd: warp-317.2")
    }
}
