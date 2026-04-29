//! Pure transport layer for talking to an `opencode serve` subprocess.
//!
//! This crate is deliberately Warp-agnostic: it owns process lifecycle, HTTP,
//! the SSE `/event` subscription (demuxed by `sessionID`), and JSON-RPC server
//! framing for plugin-initiated calls. Conversion between opencode wire types
//! and Warp's `AIClientAction` / `AgentConversationData` lives in
//! `app/src/ai/agent/opencode_backend/`.
//!
//! See `specs/opencode-as-primary-agent/TECH.md` for the full design.

pub mod http_client;
pub mod process_supervisor;
pub mod rpc_server;
pub mod schemas;
pub mod sse_demuxer;
