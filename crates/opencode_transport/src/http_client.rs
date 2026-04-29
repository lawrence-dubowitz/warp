//! Thin HTTP client wrapping `reqwest` for opencode REST endpoints.
//!
//! `OpencodeClient` is the typed front for the four endpoints the Warp
//! backend uses in Phase 1:
//!
//! - `POST /session` — create a session
//! - `POST /session/:id/prompt_async` — send a prompt (returns 204)
//! - `POST /session/:id/abort` — abort the running turn
//! - `GET /session/:id` — fetch session info
//!
//! Streaming events are handled separately in [`crate::sse_demuxer`];
//! the plugin → Warp RPC channel lives in [`crate::rpc_server`]. This
//! module is intentionally synchronous-shaped (request / response only)
//! so it can be reused from any tokio context.
//!
//! The base URL is provided by [`crate::process_supervisor`].

use reqwest::{Client, StatusCode};
use serde::de::DeserializeOwned;
use serde::Serialize;
use thiserror::Error;

use crate::schemas::{CreateSessionRequest, PromptAsyncRequest, SessionInfo};

/// Errors surfaced by [`OpencodeClient`].
///
/// `Transport` covers connection / DNS / IO; `Http` carries the body
/// for non-success status so callers can log it; `Deserialize` flags a
/// schema mismatch with upstream opencode (call out for a refresh).
#[derive(Debug, Error)]
pub enum OpencodeApiError {
    #[error("opencode http transport error: {0}")]
    Transport(#[source] reqwest::Error),

    #[error("opencode http error {status}: {body}")]
    Http { status: StatusCode, body: String },

    #[error("opencode response decode error: {0}")]
    Deserialize(#[source] reqwest::Error),
}

/// Typed front for the opencode REST surface used by the Warp backend.
///
/// One client per `opencode serve` instance. Cheap to clone: shares an
/// internal `reqwest::Client` connection pool.
#[derive(Debug, Clone)]
pub struct OpencodeClient {
    base_url: String,
    http: Client,
}

impl OpencodeClient {
    /// Build a client targeting `base_url` (e.g. `http://127.0.0.1:43217`).
    /// `base_url` must not end in a trailing slash.
    pub fn new(base_url: String) -> Self {
        Self {
            base_url,
            http: Client::new(),
        }
    }

    /// Build a client with a caller-provided `reqwest::Client`. Useful
    /// when tests need to install a custom timeout or middleware.
    pub fn with_http_client(base_url: String, http: Client) -> Self {
        Self { base_url, http }
    }

    /// `POST /session` — create a new opencode session.
    pub async fn create_session(
        &self,
        request: &CreateSessionRequest,
    ) -> Result<SessionInfo, OpencodeApiError> {
        self.post_json("/session", request).await
    }

    /// `GET /session/:id` — fetch session info.
    pub async fn get_session(&self, session_id: &str) -> Result<SessionInfo, OpencodeApiError> {
        let url = format!("{}/session/{}", self.base_url, session_id);
        let response = self
            .http
            .get(&url)
            .send()
            .await
            .map_err(OpencodeApiError::Transport)?;
        decode_response(response).await
    }

    /// `POST /session/:id/prompt_async` — enqueue a prompt for the
    /// session. Returns immediately on 204; events stream via SSE.
    pub async fn prompt_async(
        &self,
        session_id: &str,
        request: &PromptAsyncRequest,
    ) -> Result<(), OpencodeApiError> {
        let path = format!("/session/{session_id}/prompt_async");
        self.post_expect_no_body(&path, request).await
    }

    /// `POST /session/:id/abort` — request abort of the active turn.
    pub async fn abort(&self, session_id: &str) -> Result<(), OpencodeApiError> {
        let url = format!("{}/session/{}/abort", self.base_url, session_id);
        let response = self
            .http
            .post(&url)
            .send()
            .await
            .map_err(OpencodeApiError::Transport)?;
        ensure_success(response).await?;
        Ok(())
    }

    async fn post_json<B, R>(&self, path: &str, body: &B) -> Result<R, OpencodeApiError>
    where
        B: Serialize + ?Sized,
        R: DeserializeOwned,
    {
        let url = format!("{}{}", self.base_url, path);
        let response = self
            .http
            .post(&url)
            .json(body)
            .send()
            .await
            .map_err(OpencodeApiError::Transport)?;
        decode_response(response).await
    }

    async fn post_expect_no_body<B>(&self, path: &str, body: &B) -> Result<(), OpencodeApiError>
    where
        B: Serialize + ?Sized,
    {
        let url = format!("{}{}", self.base_url, path);
        let response = self
            .http
            .post(&url)
            .json(body)
            .send()
            .await
            .map_err(OpencodeApiError::Transport)?;
        ensure_success(response).await?;
        Ok(())
    }
}

async fn ensure_success(
    response: reqwest::Response,
) -> Result<reqwest::Response, OpencodeApiError> {
    let status = response.status();
    if status.is_success() {
        Ok(response)
    } else {
        let body = response.text().await.unwrap_or_default();
        Err(OpencodeApiError::Http { status, body })
    }
}

async fn decode_response<R>(response: reqwest::Response) -> Result<R, OpencodeApiError>
where
    R: DeserializeOwned,
{
    let response = ensure_success(response).await?;
    response
        .json::<R>()
        .await
        .map_err(OpencodeApiError::Deserialize)
}

#[cfg(test)]
#[path = "http_client_tests.rs"]
mod http_client_tests;
