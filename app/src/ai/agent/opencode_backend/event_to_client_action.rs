//! First-pass converter from opencode SSE events to Warp `AIClientAction`
//! batches.
//!
//! The full mapping is enumerated in
//! `specs/opencode-as-primary-agent/TECH.md` §2 (the "Stream contract"
//! table). Phase-1 highlights:
//!
//! - `session.connected` → no-op (just signals SSE connection).
//! - `session.status` busy → `AIClientAction::Init`.
//! - `message.updated` → `AddMessagesToTask` shell (no parts yet).
//! - `message.part.updated` text/reasoning → add an empty part of the
//!   right type.
//! - `message.part.delta` → `UpdateMessageText` / `UpdateReasoning`.
//! - `message.part.updated` tool=pending → record [`super::parked_call`]
//!   entry, no `ClientAction` emitted yet.
//! - tool=running → reconcile the parked entry.
//! - tool=completed/error → dedup against RPC-side result by
//!   `opencode_call_id`; emit the appropriate completion action.
//! - `session.error` → `Finished { error }`.
//! - `session.idle` → `Finished` (deduped against `session.error`).
//! - `permission.asked` → telemetry warning, auto-reject (Phase 1 only
//!   surfaces the allow-listed tools; opencode should never ask).
//! - `todo.updated` → telemetry warning, deferred to Phase 2.
//! - Unknown event → telemetry warning + `tracing::warn`, never silently
//!   dropped.
//!
//! Phase 1 stub: the function exists but is `unimplemented!()` so
//! callers can be wired without committing to a body until Stream 2
//! (`bd: warp-317.2`) does the heavy mapping.
//!
//! Spec: `specs/opencode-as-primary-agent/TECH.md` §2.
//!
//! bd: warp-317.1.6 (skeleton); warp-317.2 (real conversion).

use opencode_transport::schemas::OpencodeEvent;

/// Convert one opencode SSE event into a batch of Warp client actions.
///
/// Returns a `Vec` because a single opencode event can fan out into
/// multiple `AIClientAction` entries (e.g. a `message.updated` carrying
/// a fresh shell + N parts).
///
/// Real impl lands in `bd: warp-317.2`. The return type is left as
/// `()` here intentionally — `AIClientAction` is a sizeable enum living
/// in `app/src/ai/agent/api`, and forcing its import into the skeleton
/// would couple this stub to a churning surface. Stream 2 picks the
/// real return type when it lands.
#[allow(dead_code)]
pub(crate) fn event_to_client_action(_event: &OpencodeEvent) {
    unimplemented!("event_to_client_action — bd: warp-317.2")
}
