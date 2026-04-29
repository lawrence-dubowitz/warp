//! `AIAgentAction` ↔ opencode tool name mapping.
//!
//! Phase 1 allow-list (per `specs/opencode-as-primary-agent/TECH.md`
//! §5):
//!
//! - opencode `bash`  → `AIAgentAction::ExecuteCommand`
//! - opencode `read`  → `AIAgentAction::ReadFile`
//! - opencode `edit`  → `AIAgentAction::EditFile` (string-replace flavour)
//! - opencode `write` → `AIAgentAction::WriteFile`
//! - opencode `patch` → `AIAgentAction::EditFile` (unified-diff flavour)
//! - opencode `glob`  → `AIAgentAction::FindFiles`
//! - opencode `grep`  → `AIAgentAction::Grep`
//!
//! Everything else (`webfetch`, `todo`, `task`, `fetch`, `search`,
//! `code`, `skill`, `lsp`, `plan`, plus anything new) is shadowed by
//! the plugin and returns a "tool disabled in this build" error to
//! opencode. Belt-and-braces: opencode's permission config is also set
//! to deny non-allow-listed names.
//!
//! Phase 1 stub: types-only. Real conversion (including input-payload
//! reshaping and result canonicalisation) lands in Stream 2
//! (`bd: warp-317.2`) alongside the RPC handlers.
//!
//! Spec: `specs/opencode-as-primary-agent/TECH.md` §5.
//!
//! bd: warp-317.1.6 (skeleton); warp-317.2 (real adapter).

/// Marker for the Phase-1 allow-listed opencode tool set. Real impl
/// will be a free function `parse_action(name, input) -> Result<...>`.
#[allow(dead_code)] // Used once warp-317.2 wires the plugin.
pub(crate) struct ActionAdapter;
