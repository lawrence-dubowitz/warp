//! JSON-RPC handlers exposed to the opencode plugin.
//!
//! The plugin (a Bun TypeScript bundle in `resources/opencode-plugin/`,
//! shipped in Stream 2) shadows opencode's built-in tools and forwards
//! every allow-listed call out over JSON-RPC to Warp. This module is
//! the Warp side of those calls.
//!
//! Phase 1 surface (per `specs/opencode-as-primary-agent/TECH.md` §4):
//! - `tool.invoke(call_id, name, input)` — park + queue via
//!   `ActionModel::queue_actions(...)` so preprocessing / approval /
//!   denylist / autoexecute / cancellation all behave identically to
//!   the legacy backend.
//! - `tool.cancel(call_id)` — flip the parked entry to `Cancelled` and
//!   drain the executor.
//! - `prompt.bd_prime(conversation_id)` — return the cached read-only
//!   bd-prime memory blob (refresh every 30 min; see [`super::beads_glue`]).
//!
//! All bodies are `unimplemented!()` here. Real handlers, including the
//! shadowing/allowlist enforcement and the SSE-vs-RPC dedupe by
//! `opencode_call_id`, land in Stream 2 (`bd: warp-317.2`).
//!
//! Spec: `specs/opencode-as-primary-agent/TECH.md` §4, §6, §10.
//!
//! bd: warp-317.1.6 (skeleton); warp-317.2 (real handlers + plugin).

/// Placeholder dispatcher. Real handler trait lives in
/// `crates/opencode_transport::rpc_server` once Stream 2 lands; this
/// module will then implement that trait against an
/// [`super::backend::OpencodeBackend`].
#[allow(dead_code)] // Used once warp-317.2 wires the plugin.
pub(crate) struct RpcHandlers;
