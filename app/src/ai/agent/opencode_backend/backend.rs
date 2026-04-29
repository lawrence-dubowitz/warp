//! Top-level handle for the opencode backend.
//!
//! Holds `Arc`-shared transport primitives (lazy subprocess supervisor,
//! HTTP client to the local opencode server, and the global SSE demuxer
//! that fans events out per session). One `OpencodeBackend` per Warp app
//! is the long-term shape; the call site that constructs it is the
//! selector branch in `app/src/ai/agent/api/impl.rs` (lands in
//! `bd: warp-317.1.7`).
//!
//! Phase 1 in this skeleton: type-only. Constructor and `generate_*`
//! entry point are `unimplemented!()` until Stream 2 wires the plugin
//! RPC and Stream 3 wires persistence + bd-prime injection.
//!
//! Spec: `specs/opencode-as-primary-agent/TECH.md` §1, §7.
//!
//! bd: warp-317.1.6 (skeleton); warp-317.1.7 (selector wiring);
//! warp-317.2 (RPC + parked-call machine); warp-317.3 (persistence + bd
//! prime + cancellation plumbing).

use std::sync::Arc;

use opencode_transport::http_client::OpencodeClient;
use opencode_transport::process_supervisor::Supervisor;
use opencode_transport::sse_demuxer::SseDemuxer;

/// Shared handle for the opencode backend.
///
/// Cheap to clone (all fields are `Arc`); intended to live for the
/// lifetime of the Warp app instance. A future real constructor will
/// own:
/// - `supervisor`: lazy-started `opencode serve` subprocess; one per
///   Warp app, killed on quit.
/// - `demuxer`: single global SSE `/event` subscription, demuxed by
///   `sessionID`.
/// - `http`: thin reqwest wrapper for opencode's REST endpoints.
///
/// Fields are `pub(super)` so sibling modules in this directory can
/// reach them when their real implementations land.
#[derive(Clone)]
#[allow(dead_code)] // Fields read once Stream 2 (warp-317.2) wires real handlers.
pub(crate) struct OpencodeBackend {
    pub(super) supervisor: Arc<Supervisor>,
    pub(super) demuxer: Arc<SseDemuxer>,
    pub(super) http: Arc<OpencodeClient>,
}

impl OpencodeBackend {
    /// Construct the backend handle.
    ///
    /// Real wiring lands in `bd: warp-317.1.7` (selector branch). The
    /// signature here is intentionally absent until that task picks the
    /// concrete `AppContext` / config inputs.
    #[allow(dead_code)]
    pub(crate) fn new(
        supervisor: Arc<Supervisor>,
        demuxer: Arc<SseDemuxer>,
        http: Arc<OpencodeClient>,
    ) -> Self {
        Self {
            supervisor,
            demuxer,
            http,
        }
    }
}
