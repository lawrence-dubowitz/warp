//! Single global SSE `/event` subscription demuxed per `sessionID`.
//!
//! ## Design
//!
//! - **One SSE connection** to opencode's `GET /event`, regardless of how many
//!   sessions are subscribed.
//! - **Per-session unbounded `async_channel`s**. Each `subscribe(session_id)`
//!   call hands out a fresh `Receiver`; the matching `Sender` lives in a
//!   shared registry keyed by session id.
//! - **Reconnect with exponential backoff** modeled on
//!   `app/src/ai/agent_events/driver.rs`. The registry survives reconnects:
//!   subscribers do **not** see channel closure when the SSE stream dies.
//! - **Lazy producer**. The SSE task is spawned on the first `subscribe()`
//!   and cancelled on `shutdown()` or demuxer drop.
//!
//! ## Why per-session `async_channel` rather than `broadcast`?
//!
//! The workspace house style (verified zero `tokio::sync::broadcast`
//! callsites) is `async_broadcast` for shared fan-out and dedicated
//! `async_channel`s for per-key subscribers. Routing here is per-`sessionID`,
//! not shared, so each subscriber gets its own mpsc queue.
//!
//! ## What gets dropped
//!
//! Events whose `properties.sessionID` is missing (heartbeats,
//! `server.connected`, project/lsp events) or unregistered are logged at
//! `debug` and dropped. The demuxer is a routing layer, not a buffering one.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use async_channel::{Receiver, Sender};
use futures_util::StreamExt;
use parking_lot::Mutex;
use reqwest_eventsource::{Event, RequestBuilderExt};
use tokio::task::JoinHandle;

use crate::process_supervisor::HttpEndpoint;
use crate::schemas::OpencodeEvent;

const RECONNECT_BACKOFF_INIT: Duration = Duration::from_millis(200);
const RECONNECT_BACKOFF_MAX: Duration = Duration::from_secs(5);
/// Continuous-connected time after which the next failure resets backoff to
/// [`RECONNECT_BACKOFF_INIT`]. Mirrors `agent_events/driver.rs`.
const RECONNECT_HEALTHY_THRESHOLD: Duration = Duration::from_secs(60);

type Registry = Arc<Mutex<HashMap<String, Sender<OpencodeEvent>>>>;

/// Cheap-to-clone handle to a single SSE subscription routed by session id.
///
/// Cloning shares the underlying registry and producer task. Drop the last
/// clone (or call [`SseDemuxer::shutdown`]) to terminate the SSE connection.
#[derive(Clone)]
pub struct SseDemuxer {
    inner: Arc<DemuxerInner>,
}

struct DemuxerInner {
    endpoint: HttpEndpoint,
    registry: Registry,
    producer: tokio::sync::Mutex<ProducerState>,
}

#[derive(Default)]
struct ProducerState {
    handle: Option<JoinHandle<()>>,
    shutdown: Option<Sender<()>>,
}

impl SseDemuxer {
    /// Build a demuxer pointed at `endpoint`'s `/event` route.
    ///
    /// The SSE subscription is **not** opened until the first call to
    /// [`subscribe`](Self::subscribe).
    pub fn new(endpoint: HttpEndpoint) -> Self {
        Self {
            inner: Arc::new(DemuxerInner {
                endpoint,
                registry: Arc::new(Mutex::new(HashMap::new())),
                producer: tokio::sync::Mutex::new(ProducerState::default()),
            }),
        }
    }

    /// Subscribe to events for `session_id`. Returns an unbounded receiver.
    ///
    /// Re-subscribing the same session id replaces the previous sender; the
    /// old receiver observes channel closure. The producer task is started
    /// lazily on the first call.
    pub async fn subscribe(&self, session_id: impl Into<String>) -> Receiver<OpencodeEvent> {
        let session_id = session_id.into();
        let (tx, rx) = async_channel::unbounded();
        {
            let mut registry = self.inner.registry.lock();
            if let Some(prev) = registry.insert(session_id, tx) {
                prev.close();
            }
        }
        self.ensure_producer_started().await;
        rx
    }

    /// Drop the sender for `session_id`. The receiver observes channel
    /// closure on its next poll.
    pub fn unsubscribe(&self, session_id: &str) {
        let mut registry = self.inner.registry.lock();
        if let Some(sender) = registry.remove(session_id) {
            sender.close();
        }
    }

    /// Cancel the SSE subscription and close every registered receiver.
    ///
    /// Safe to call from any clone; subsequent `subscribe()` calls will
    /// restart the producer. After `shutdown()` returns the SSE task is
    /// guaranteed to have stopped polling the network.
    pub async fn shutdown(&self) {
        let (handle, shutdown) = {
            let mut producer = self.inner.producer.lock().await;
            (producer.handle.take(), producer.shutdown.take())
        };
        if let Some(tx) = shutdown {
            let _ = tx.try_send(());
            tx.close();
        }
        if let Some(handle) = handle {
            handle.abort();
            let _ = handle.await;
        }
        let mut registry = self.inner.registry.lock();
        for (_, sender) in registry.drain() {
            sender.close();
        }
    }

    async fn ensure_producer_started(&self) {
        let mut producer = self.inner.producer.lock().await;
        if producer
            .handle
            .as_ref()
            .map(|h| !h.is_finished())
            .unwrap_or(false)
        {
            return;
        }
        let (shutdown_tx, shutdown_rx) = async_channel::bounded::<()>(1);
        let endpoint = self.inner.endpoint.clone();
        let registry = self.inner.registry.clone();
        let handle = tokio::spawn(async move {
            run_producer(endpoint, registry, shutdown_rx).await;
        });
        producer.handle = Some(handle);
        producer.shutdown = Some(shutdown_tx);
    }
}

impl Drop for DemuxerInner {
    fn drop(&mut self) {
        // Best-effort: abort any in-flight producer task so we don't leak it
        // if the runtime outlives this demuxer. We can't `await` here, so we
        // settle for `abort()` and rely on `shutdown()` for the synchronous
        // path.
        if let Ok(mut producer) = self.producer.try_lock() {
            if let Some(handle) = producer.handle.take() {
                handle.abort();
            }
            if let Some(tx) = producer.shutdown.take() {
                tx.close();
            }
        }
    }
}

async fn run_producer(endpoint: HttpEndpoint, registry: Registry, shutdown_rx: Receiver<()>) {
    let url = format!("{}/event", endpoint.base_url);
    let client = reqwest::Client::new();
    let mut backoff = RECONNECT_BACKOFF_INIT;

    loop {
        if shutdown_rx.is_closed() {
            return;
        }

        let connected_at = instant::Instant::now();
        let stream_result = client.get(&url).eventsource();
        let mut stream = match stream_result {
            Ok(s) => s,
            Err(err) => {
                tracing::warn!(
                    %url,
                    error = %err,
                    "failed to build opencode SSE stream; retrying after backoff",
                );
                if wait_or_shutdown(backoff, &shutdown_rx).await {
                    return;
                }
                backoff = next_backoff(backoff);
                continue;
            }
        };

        let mut healthy = false;
        loop {
            tokio::select! {
                _ = shutdown_rx.recv() => {
                    stream.close();
                    return;
                }
                event = stream.next() => {
                    match event {
                        Some(Ok(Event::Open)) => {
                            tracing::info!(%url, "opencode SSE connection open");
                            backoff = RECONNECT_BACKOFF_INIT;
                            healthy = true;
                        }
                        Some(Ok(Event::Message(message))) => {
                            healthy = true;
                            dispatch_message(&message.data, &registry);
                        }
                        Some(Err(err)) => {
                            if matches!(err, reqwest_eventsource::Error::StreamEnded) {
                                tracing::debug!(%url, "opencode SSE stream ended; reconnecting");
                            } else {
                                tracing::warn!(%url, error = %err, "opencode SSE error; reconnecting");
                            }
                            stream.close();
                            break;
                        }
                        None => {
                            tracing::debug!(%url, "opencode SSE stream closed; reconnecting");
                            break;
                        }
                    }
                }
            }
        }

        if healthy && connected_at.elapsed() >= RECONNECT_HEALTHY_THRESHOLD {
            backoff = RECONNECT_BACKOFF_INIT;
        }
        if wait_or_shutdown(backoff, &shutdown_rx).await {
            return;
        }
        backoff = next_backoff(backoff);
    }
}

fn dispatch_message(data: &str, registry: &Registry) {
    let event: OpencodeEvent = match serde_json::from_str(data) {
        Ok(ev) => ev,
        Err(err) => {
            tracing::warn!(
                error = %err,
                payload = %truncate(data, 256),
                "skipping malformed opencode SSE event",
            );
            return;
        }
    };

    let Some(session_id) = event.session_id().map(str::to_owned) else {
        tracing::debug!(event_type = %event.r#type, "dropping non-session opencode event");
        return;
    };

    let sender = {
        let registry = registry.lock();
        registry.get(&session_id).cloned()
    };

    let Some(sender) = sender else {
        tracing::debug!(
            event_type = %event.r#type,
            %session_id,
            "dropping event for unregistered session",
        );
        return;
    };

    if let Err(err) = sender.try_send(event) {
        tracing::warn!(
            %session_id,
            error = %err,
            "failed to deliver opencode event to subscriber",
        );
    }
}

fn next_backoff(current: Duration) -> Duration {
    let doubled = current.saturating_mul(2);
    if doubled > RECONNECT_BACKOFF_MAX {
        RECONNECT_BACKOFF_MAX
    } else {
        doubled
    }
}

async fn wait_or_shutdown(delay: Duration, shutdown_rx: &Receiver<()>) -> bool {
    tokio::select! {
        _ = tokio::time::sleep(delay) => false,
        _ = shutdown_rx.recv() => true,
    }
}

fn truncate(s: &str, max: usize) -> &str {
    if s.len() <= max {
        s
    } else {
        &s[..max]
    }
}

#[cfg(test)]
#[path = "sse_demuxer_tests.rs"]
mod sse_demuxer_tests;
