//! Builds the system + user prompt sent to opencode.
//!
//! Phase 1 strategy: pass the Warp prompt through unchanged. Once the
//! backend is functional we'll layer on opencode-specific instructions
//! (allow-listed tools, no-MCP-resources reminder, bd-prime block
//! injected from [`super::beads_glue`], etc.).
//!
//! Real impl lands in Stream 3 (`bd: warp-317.3`) where bd-prime
//! injection is wired.
//!
//! Spec: `specs/opencode-as-primary-agent/TECH.md` §6, §10.
//!
//! bd: warp-317.1.6 (skeleton); warp-317.3 (bd prime + final prompt
//! shaping).

/// Build the prompt blob handed to `opencode_transport::http_client::
/// OpencodeClient::prompt_async`.
///
/// Phase 1: returns the input unchanged.
#[allow(dead_code)]
pub(crate) fn build_prompt(prompt: String) -> String {
    prompt
}
