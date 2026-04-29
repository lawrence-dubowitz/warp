//! Read-only `bd prime` memory injection for opencode prompts.
//!
//! When a conversation runs on the opencode backend, we inject the
//! output of `bd prime` (read-only) into the system prompt so the
//! agent has access to the current beads project memory the same way
//! a human running `bd prime` in their shell does.
//!
//! Caching strategy (per
//! `specs/opencode-as-primary-agent/TECH.md` §10):
//! - Cache keyed per Warp conversation.
//! - Refresh on app start.
//! - Refresh again every 30 minutes.
//! - Missing `bd` binary → `tracing::warn!` once, then skip injection
//!   for the rest of the app session (do not poison every prompt with
//!   an error blob).
//!
//! Phase 1 stub: the function exists but always returns `None`. Real
//! shelling-out to `bd prime` lands in Stream 3 (`bd: warp-317.3`).
//!
//! Spec: `specs/opencode-as-primary-agent/TECH.md` §10.
//!
//! bd: warp-317.1.6 (skeleton); warp-317.3 (real cache + invocation).

/// Return the cached `bd prime` blob for the given conversation, if
/// available. Phase 1 always returns `None`.
#[allow(dead_code)]
pub(crate) fn cached_bd_prime(_conversation_id: &str) -> Option<String> {
    None
}
