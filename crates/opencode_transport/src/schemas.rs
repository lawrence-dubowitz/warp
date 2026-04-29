//! Vendored opencode wire schemas (sessions, parts, errors).
//!
//! These mirror the upstream opencode types as published at the
//! HTTP boundary. They are intentionally minimal: only the fields
//! Warp consumes today are modeled. Optional fields default and
//! unknown fields are ignored so opencode upgrades that add fields
//! do not break the client.
//!
//! Tracked against opencode commit `a3f7ea2`. When upstream evolves,
//! refresh this module rather than letting Warp domain types leak in.
//! See `specs/opencode-as-primary-agent/TECH.md` for context.

use serde::{Deserialize, Serialize};

/// `Session.Info` returned by `POST /session` and `GET /session/:id`.
///
/// Only the fields the Warp backend reads are modeled. Additional
/// fields opencode emits (e.g. `share`, `revert`) are accepted but
/// dropped on deserialize.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionInfo {
    /// Session identifier (used in subsequent `/session/:id/...` calls).
    pub id: String,
    /// Optional human-readable title.
    #[serde(default)]
    pub title: Option<String>,
    /// Optional opencode-assigned slug.
    #[serde(default)]
    pub slug: Option<String>,
    /// Project identifier (`projectID` upstream).
    #[serde(default, rename = "projectID")]
    pub project_id: Option<String>,
    /// Workspace identifier (`workspaceID` upstream).
    #[serde(default, rename = "workspaceID")]
    pub workspace_id: Option<String>,
    /// Working directory the session is bound to.
    #[serde(default)]
    pub directory: Option<String>,
    /// Parent session identifier (`parentID` upstream).
    #[serde(default, rename = "parentID")]
    pub parent_id: Option<String>,
    /// Free-form summary text (model-authored).
    #[serde(default)]
    pub summary: Option<String>,
    /// Schema version reported by opencode for this session record.
    #[serde(default)]
    pub version: Option<String>,
}

/// Body for `POST /session`. All fields are optional; sending an empty
/// object accepts opencode's defaults, which is the common path.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CreateSessionRequest {
    #[serde(skip_serializing_if = "Option::is_none", rename = "parentID")]
    pub parent_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// Optional permission ruleset; if absent opencode uses its plugin
    /// configuration. Warp leaves this empty in Phase 1 and relies on
    /// the bundled plugin's allowlist instead.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub permission: Option<Vec<PermissionRule>>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "workspaceID")]
    pub workspace_id: Option<String>,
}

/// Single rule inside a `Permission.Ruleset`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionRule {
    pub permission: String,
    pub pattern: String,
    pub action: PermissionAction,
}

/// Permission action verbs accepted by opencode.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PermissionAction {
    Allow,
    Deny,
    Ask,
}

/// Body for `POST /session/:id/prompt_async`.
///
/// The `sessionID` field on `PromptInput` upstream is supplied via the
/// URL path, not this body. Phase 1 only sends `parts` (text), `model`,
/// `agent`, and optional `messageID`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PromptAsyncRequest {
    #[serde(skip_serializing_if = "Option::is_none", rename = "messageID")]
    pub message_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "noReply")]
    pub no_reply: Option<bool>,
    /// Per-prompt tool gating. Map of opencode tool name → enabled.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<std::collections::HashMap<String, bool>>,
    /// Output format selector. Phase 1 leaves this unset (text default).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub format: Option<Format>,
    pub parts: Vec<PromptPart>,
}

/// `format` selector on `PromptInput`. Phase 1 only emits text; the
/// `JsonSchema` variant is reserved for future structured-output flows.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Format {
    Text,
    JsonSchema {
        schema: serde_json::Value,
        #[serde(skip_serializing_if = "Option::is_none", rename = "retryCount")]
        retry_count: Option<u32>,
    },
}

/// Discriminated `parts[]` entry. Phase 1 only emits `Text`; file/
/// agent/subtask variants are deferred until the corresponding Warp
/// surfaces are wired through.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PromptPart {
    Text { text: String },
}

/// 400-error envelope opencode returns from validation failures.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BadRequestError {
    pub success: bool,
    pub data: serde_json::Value,
    pub errors: Vec<serde_json::Value>,
}

/// 404-error envelope opencode returns when a session id is unknown.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotFoundError {
    pub name: String,
    pub data: NotFoundErrorData,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotFoundErrorData {
    pub message: String,
}

/// Bus-event envelope written to `GET /event` SSE frames.
///
/// Upstream shape (`packages/opencode/src/bus/bus-event.ts`):
///   `{ type: <literal>, properties: <typed> }`
///
/// Session-bearing events carry `properties.sessionID`. Non-session events
/// (e.g. `server.connected`, `server.heartbeat`, `lsp.client.diagnostics`)
/// have no `sessionID` in their properties and are dropped by the demuxer.
///
/// `properties` is intentionally a `serde_json::Value` for forward-compat:
/// the 47-variant upstream union grows over time, and the demuxer only
/// needs `sessionID` for routing. Backend consumers that care about a
/// specific event variant deserialize `properties` into a typed struct.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpencodeEvent {
    /// Event discriminant, e.g. `"message.part.delta"` or `"session.idle"`.
    #[serde(rename = "type")]
    pub r#type: String,
    /// Event-specific payload. Schema varies by `type`.
    pub properties: serde_json::Value,
}

impl OpencodeEvent {
    /// Returns `properties.sessionID` if the event carries one.
    ///
    /// Used by `sse_demuxer` to route events to per-session subscribers.
    pub fn session_id(&self) -> Option<&str> {
        self.properties.get("sessionID").and_then(|v| v.as_str())
    }
}
