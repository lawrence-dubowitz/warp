//! opencode-as-primary-agent backend: Warp-side glue around the
//! `opencode_transport` crate.
//!
//! This module is the Warp half of the integration. The pure transport
//! (process supervisor, HTTP client, SSE demuxer, vendored schemas) lives
//! in `crates/opencode_transport`; everything in this directory translates
//! between opencode wire types and Warp's existing agent runtime
//! (`AIAgentAction`, `AIClientAction`, `ParkedToolCall`-style execution).
//!
//! Spec: `specs/opencode-as-primary-agent/TECH.md`.
//!
//! Submodules (each gated to its own bd task):
//! - [`backend`] — top-level handle, holds Arc<Supervisor> + Arc<SseDemuxer>
//!   + Arc<OpencodeClient>. Currently stubs; real wiring in
//!   `bd: warp-317.1.7` (selector branch in `generate_multi_agent_output`).
//! - [`session_router`] — Warp `conversation_id` ↔ opencode `session_id`
//!   mapping, lazy-creates an opencode session per Warp conversation.
//! - [`event_to_client_action`] — first-pass converter for SSE events
//!   into the Warp `AIClientAction` stream. See TECH.md §2 mapping table.
//! - [`parked_call`] — per-tool-call state machine that parks opencode tool
//!   invocations on the Warp side until preprocess/approval/execution
//!   finishes. Full machine lands in Stream 2 (`bd: warp-317.2`).
//! - [`rpc_handlers`] — handlers for the JSON-RPC the bun plugin calls
//!   into Warp on. Stubs only; real handlers in Stream 2.
//! - [`action_adapter`] — `AIAgentAction` ↔ opencode tool-name mapping
//!   used by RPC handlers + `event_to_client_action`.
//! - [`prompt_builder`] — Phase 1 returns the existing Warp prompt
//!   unchanged; bd-prime injection lands in Stream 3 (`bd: warp-317.3`).
//! - [`persistence_glue`] — helpers for the new JSON fields on
//!   `AgentConversationData` (`backend_kind`, `opencode_session_id`,
//!   `opencode_id_map`). Real fields added in Stream 3.
//! - [`beads_glue`] — `bd prime` read-only memory injection. Stub now,
//!   real impl in Stream 3.
//!
//! bd: warp-317.1.6 (skeleton).

pub(crate) mod action_adapter;
pub(crate) mod backend;
pub(crate) mod beads_glue;
pub(crate) mod event_to_client_action;
pub(crate) mod parked_call;
pub(crate) mod persistence_glue;
pub(crate) mod prompt_builder;
pub(crate) mod rpc_handlers;
pub(crate) mod session_router;
