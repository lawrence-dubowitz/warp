//! JSON-RPC 2.0 server for plugin → Warp tool-call dispatch.
//!
//! Listens on a local TCP port. Accepts a single connection from the opencode
//! plugin process. Reads newline-delimited JSON-RPC 2.0 messages, dispatches to
//! a [`Handler`] implementation, and writes back responses.
//!
//! This is a pure-transport module with no Warp-app types. The handler trait is
//! generic over request/response payloads (serde_json::Value). Warp-side
//! dispatch logic lives in `app/src/ai/agent/opencode_backend/rpc_handlers.rs`.
//!
//! Design decisions:
//! - TCP (not stdio) because the plugin is a child process whose stdio is
//!   consumed by opencode's own protocol. A local TCP loopback socket is the
//!   simplest side-channel.
//! - Single connection: one plugin process per Warp instance, connected for the
//!   lifetime of the opencode subprocess.
//! - Newline-delimited framing (each message is a single JSON object followed
//!   by `\n`). Matches the LSP base protocol's Content-Length framing in spirit
//!   but simpler — plugin and server are co-located on localhost with no
//!   streaming bodies.
//!
//! bd: warp-317.2.1

use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Mutex;
use tokio::task::JoinHandle;
use tracing::{debug, error, info, warn};

// ─── JSON-RPC 2.0 wire types ───────────────────────────────────────────────

/// JSON-RPC 2.0 version string.
const JSONRPC_VERSION: &str = "2.0";

/// Incoming JSON-RPC request (has `id` → expects response).
#[derive(Debug, Clone, Deserialize)]
pub struct RpcRequest {
    pub jsonrpc: String,
    pub id: RpcId,
    pub method: String,
    #[serde(default)]
    pub params: Value,
}

/// Incoming JSON-RPC notification (no `id` → no response).
#[derive(Debug, Clone, Deserialize)]
pub struct RpcNotification {
    pub jsonrpc: String,
    pub method: String,
    #[serde(default)]
    pub params: Value,
}

/// JSON-RPC request id — integer or string per spec.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(untagged)]
pub enum RpcId {
    Integer(i64),
    String(String),
}

/// Outgoing JSON-RPC success response.
#[derive(Debug, Serialize)]
pub struct RpcResponse {
    pub jsonrpc: &'static str,
    pub id: RpcId,
    pub result: Value,
}

/// Outgoing JSON-RPC error response.
#[derive(Debug, Serialize)]
pub struct RpcErrorResponse {
    pub jsonrpc: &'static str,
    pub id: RpcId,
    pub error: RpcError,
}

/// JSON-RPC error object.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcError {
    pub code: i64,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

/// Standard JSON-RPC error codes.
pub mod error_codes {
    pub const PARSE_ERROR: i64 = -32700;
    pub const INVALID_REQUEST: i64 = -32600;
    pub const METHOD_NOT_FOUND: i64 = -32601;
    pub const INVALID_PARAMS: i64 = -32602;
    pub const INTERNAL_ERROR: i64 = -32603;
}

// ─── Envelope for parsing ───────────────────────────────────────────────────

/// Raw envelope used to distinguish requests from notifications.
/// If `id` is present → request; absent → notification.
#[derive(Debug, Deserialize)]
struct RawEnvelope {
    #[allow(dead_code)]
    jsonrpc: Option<String>,
    id: Option<RpcId>,
    method: Option<String>,
    #[serde(default)]
    params: Value,
}

// ─── Handler trait ──────────────────────────────────────────────────────────

/// Result type returned by handler methods.
pub type HandlerResult = std::result::Result<Value, RpcError>;

/// Trait implemented by Warp-side dispatch logic (rpc_handlers.rs).
///
/// Each method receives the raw `params` as a serde_json::Value.
/// Return `Ok(Value)` for success or `Err(RpcError)` for structured errors.
#[async_trait::async_trait]
pub trait Handler: Send + Sync + 'static {
    /// Handle a request (has id, expects response).
    async fn handle_request(&self, method: &str, params: Value) -> HandlerResult;

    /// Handle a notification (no id, no response). Default: log and ignore.
    async fn handle_notification(&self, method: &str, params: Value) {
        debug!(method, "received notification (no handler registered)");
        let _ = params;
    }
}

// ─── Server ─────────────────────────────────────────────────────────────────

/// Errors from [`RpcServer`] operations.
#[derive(Debug, Error)]
pub enum RpcServerError {
    #[error("failed to bind TCP listener: {0}")]
    Bind(#[source] std::io::Error),

    #[error("server is shut down")]
    ShutDown,
}

/// A running JSON-RPC server listening on a local TCP port.
///
/// Accepts a single connection from the opencode plugin. If the connection
/// drops, the server waits for a new one (supports plugin restart without
/// Warp restart).
#[derive(Clone)]
pub struct RpcServer {
    inner: Arc<RpcServerInner>,
}

struct RpcServerInner {
    addr: SocketAddr,
    state: Mutex<ServerState>,
}

struct ServerState {
    handle: Option<JoinHandle<()>>,
    shutdown_tx: Option<tokio::sync::oneshot::Sender<()>>,
}

impl RpcServer {
    /// Start the RPC server on a random local port.
    ///
    /// The `handler` receives all incoming requests/notifications.
    /// Returns immediately; the server runs in a background tokio task.
    pub async fn start(handler: Arc<dyn Handler>) -> Result<Self, RpcServerError> {
        Self::start_on("127.0.0.1:0", handler).await
    }

    /// Start on a specific address (useful for tests with fixed ports).
    pub async fn start_on(addr: &str, handler: Arc<dyn Handler>) -> Result<Self, RpcServerError> {
        let listener = TcpListener::bind(addr)
            .await
            .map_err(RpcServerError::Bind)?;
        let bound_addr = listener.local_addr().map_err(RpcServerError::Bind)?;

        info!(%bound_addr, "RPC server listening");

        let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();

        let handle = tokio::spawn(run_accept_loop(listener, handler, shutdown_rx));

        Ok(Self {
            inner: Arc::new(RpcServerInner {
                addr: bound_addr,
                state: Mutex::new(ServerState {
                    handle: Some(handle),
                    shutdown_tx: Some(shutdown_tx),
                }),
            }),
        })
    }

    /// The address the server is listening on.
    pub fn addr(&self) -> SocketAddr {
        self.inner.addr
    }

    /// The port the server is listening on.
    pub fn port(&self) -> u16 {
        self.inner.addr.port()
    }

    /// Shut down the server gracefully.
    pub async fn shutdown(&self) {
        let mut state = self.inner.state.lock().await;
        if let Some(tx) = state.shutdown_tx.take() {
            let _ = tx.send(());
        }
        if let Some(handle) = state.handle.take() {
            let _ = handle.await;
        }
        info!("RPC server shut down");
    }
}

// ─── Server internals ───────────────────────────────────────────────────────

async fn run_accept_loop(
    listener: TcpListener,
    handler: Arc<dyn Handler>,
    mut shutdown_rx: tokio::sync::oneshot::Receiver<()>,
) {
    loop {
        tokio::select! {
            biased;

            _ = &mut shutdown_rx => {
                debug!("RPC server received shutdown signal");
                break;
            }

            accept_result = listener.accept() => {
                match accept_result {
                    Ok((stream, peer)) => {
                        info!(%peer, "plugin connected");
                        handle_connection(stream, handler.clone(), &mut shutdown_rx).await;
                        info!("plugin disconnected, waiting for reconnect");
                    }
                    Err(e) => {
                        error!(error = %e, "accept failed");
                        // Brief pause before retrying accept
                        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                    }
                }
            }
        }
    }
}

async fn handle_connection(
    stream: TcpStream,
    handler: Arc<dyn Handler>,
    shutdown_rx: &mut tokio::sync::oneshot::Receiver<()>,
) {
    let (reader, writer) = stream.into_split();
    let mut lines = BufReader::new(reader).lines();
    let writer = Arc::new(Mutex::new(writer));

    loop {
        tokio::select! {
            biased;

            _ = &mut *shutdown_rx => {
                debug!("connection loop interrupted by shutdown");
                break;
            }

            line_result = lines.next_line() => {
                match line_result {
                    Ok(Some(line)) => {
                        if line.trim().is_empty() {
                            continue;
                        }
                        let handler_clone = handler.clone();
                        let writer_clone = writer.clone();
                        // Spawn per-message to allow concurrent requests
                        tokio::spawn(async move {
                            process_message(&line, handler_clone, writer_clone).await;
                        });
                    }
                    Ok(None) => {
                        // Connection closed cleanly
                        break;
                    }
                    Err(e) => {
                        warn!(error = %e, "read error on plugin connection");
                        break;
                    }
                }
            }
        }
    }
}

async fn process_message(
    line: &str,
    handler: Arc<dyn Handler>,
    writer: Arc<Mutex<tokio::net::tcp::OwnedWriteHalf>>,
) {
    let envelope: RawEnvelope = match serde_json::from_str(line) {
        Ok(env) => env,
        Err(e) => {
            warn!(error = %e, "failed to parse JSON-RPC envelope");
            // Send parse error with null id
            let resp = RpcErrorResponse {
                jsonrpc: JSONRPC_VERSION,
                id: RpcId::Integer(0),
                error: RpcError {
                    code: error_codes::PARSE_ERROR,
                    message: format!("parse error: {e}"),
                    data: None,
                },
            };
            write_response(&writer, &resp).await;
            return;
        }
    };

    let method = match envelope.method {
        Some(m) => m,
        None => {
            // No method field — invalid request
            if let Some(id) = envelope.id {
                let resp = RpcErrorResponse {
                    jsonrpc: JSONRPC_VERSION,
                    id,
                    error: RpcError {
                        code: error_codes::INVALID_REQUEST,
                        message: "missing 'method' field".to_string(),
                        data: None,
                    },
                };
                write_response(&writer, &resp).await;
            }
            return;
        }
    };

    match envelope.id {
        Some(id) => {
            // Request — dispatch and respond
            let result = handler.handle_request(&method, envelope.params).await;
            match result {
                Ok(value) => {
                    let resp = RpcResponse {
                        jsonrpc: JSONRPC_VERSION,
                        id,
                        result: value,
                    };
                    write_response(&writer, &resp).await;
                }
                Err(rpc_err) => {
                    let resp = RpcErrorResponse {
                        jsonrpc: JSONRPC_VERSION,
                        id,
                        error: rpc_err,
                    };
                    write_response(&writer, &resp).await;
                }
            }
        }
        None => {
            // Notification — no response
            handler.handle_notification(&method, envelope.params).await;
        }
    }
}

async fn write_response<T: Serialize>(
    writer: &Arc<Mutex<tokio::net::tcp::OwnedWriteHalf>>,
    response: &T,
) {
    let mut buf = match serde_json::to_vec(response) {
        Ok(b) => b,
        Err(e) => {
            error!(error = %e, "failed to serialize RPC response");
            return;
        }
    };
    buf.push(b'\n');

    let mut guard = writer.lock().await;
    if let Err(e) = guard.write_all(&buf).await {
        warn!(error = %e, "failed to write RPC response");
    }
}

#[cfg(test)]
#[path = "rpc_server_tests.rs"]
mod rpc_server_tests;
