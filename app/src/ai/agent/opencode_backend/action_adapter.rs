#![allow(dead_code, clippy::single_range_in_vec_init)]
//! `AIAgentAction` ↔ opencode tool name mapping.
//!
//! Phase 1 allow-list (per `specs/opencode-as-primary-agent/TECH.md`
//! §5):
//!
//! - opencode `bash`  → `AIAgentActionType::RequestCommandOutput`
//! - opencode `read`  → `AIAgentActionType::ReadFiles`
//! - opencode `edit`  → `AIAgentActionType::RequestFileEdits` (string-replace → synthetic diff)
//! - opencode `write` → `AIAgentActionType::RequestFileEdits` (Create variant)
//! - opencode `patch` → `AIAgentActionType::RequestFileEdits` (unified-diff)
//! - opencode `glob`  → `AIAgentActionType::FileGlob`
//! - opencode `grep`  → `AIAgentActionType::Grep`
//!
//! Everything else (`webfetch`, `todo`, `task`, `fetch`, `search`,
//! `code`, `skill`, `lsp`, `plan`, plus anything new) is shadowed by
//! the plugin and returns a "tool disabled in this build" error to
//! opencode. Belt-and-braces: opencode's permission config is also set
//! to deny non-allow-listed names.
//!
//! Spec: `specs/opencode-as-primary-agent/TECH.md` §5.
//!
//! bd: warp-317.1.6 (skeleton); warp-317.2.3 (this impl).

use ai::agent::{
    action::{AIAgentActionType, FileEdit, ReadFilesRequest},
    FileLocations,
};
use ai::diff_validation::{ParsedDiff, V4AHunk};
use serde_json::Value;

/// The set of tool names that Phase 1 allows through from the opencode
/// plugin. The plugin shadows ALL opencode built-ins; only these names
/// produce real `AIAgentAction` dispatches. Everything else returns a
/// "tool disabled" error from the plugin without reaching Warp.
pub(crate) const ALLOWED_TOOLS: &[&str] =
    &["bash", "read", "edit", "write", "patch", "glob", "grep"];

/// Returns `true` if `tool_name` is in the Phase-1 allow-list.
pub(crate) fn is_allowed(tool_name: &str) -> bool {
    ALLOWED_TOOLS.contains(&tool_name)
}

/// Error type for tool-input parsing failures.
#[derive(Debug, thiserror::Error)]
pub(crate) enum AdapterError {
    #[error("unknown or disabled tool: {0}")]
    DisabledTool(String),
    #[error("missing required field `{field}` for tool `{tool}`")]
    MissingField {
        tool: &'static str,
        field: &'static str,
    },
    #[error("invalid field `{field}` for tool `{tool}`: {reason}")]
    InvalidField {
        tool: &'static str,
        field: &'static str,
        reason: String,
    },
}

/// Parse opencode tool invocation into an `AIAgentActionType`.
///
/// `tool_name` is the opencode tool name (e.g. `"bash"`).
/// `input` is the JSON object the plugin received from opencode's
/// tool-call payload (`arguments` field, already parsed).
///
/// Returns the corresponding `AIAgentActionType` ready for wrapping
/// into an `AIAgentAction` and queueing via `ActionModel::queue_actions`.
pub(crate) fn parse_action(
    tool_name: &str,
    input: &Value,
) -> Result<AIAgentActionType, AdapterError> {
    match tool_name {
        "bash" => parse_bash(input),
        "read" => parse_read(input),
        "edit" => parse_edit(input),
        "write" => parse_write(input),
        "patch" => parse_patch(input),
        "glob" => parse_glob(input),
        "grep" => parse_grep(input),
        other => Err(AdapterError::DisabledTool(other.to_owned())),
    }
}

// ── bash ─────────────────────────────────────────────────────────────

fn parse_bash(input: &Value) -> Result<AIAgentActionType, AdapterError> {
    let command = require_str(input, "bash", "command")?;
    Ok(AIAgentActionType::RequestCommandOutput {
        command: command.to_owned(),
        // opencode doesn't supply risk/readonly metadata; leave for
        // Warp's preprocess pass to decide.
        is_read_only: None,
        is_risky: None,
        wait_until_completion: true,
        uses_pager: None,
        rationale: None,
        citations: Vec::new(),
    })
}

// ── read ─────────────────────────────────────────────────────────────

fn parse_read(input: &Value) -> Result<AIAgentActionType, AdapterError> {
    let file_path = require_str(input, "read", "file_path")?;
    let offset = input
        .get("offset")
        .and_then(Value::as_u64)
        .map(|v| v as usize);
    let limit = input
        .get("limit")
        .and_then(Value::as_u64)
        .map(|v| v as usize);

    let lines = match (offset, limit) {
        (Some(start), Some(count)) => vec![start..(start + count)],
        (Some(start), None) => vec![start..usize::MAX],
        (None, Some(count)) => vec![1..count],
        (None, None) => Vec::new(),
    };

    Ok(AIAgentActionType::ReadFiles(ReadFilesRequest {
        locations: vec![FileLocations {
            name: file_path.to_owned(),
            lines,
        }],
    }))
}

// ── edit (string-replace) ────────────────────────────────────────────

fn parse_edit(input: &Value) -> Result<AIAgentActionType, AdapterError> {
    let file_path = require_str(input, "edit", "file_path")?;
    let old_string = require_str(input, "edit", "old_string")?;
    let new_string = require_str(input, "edit", "new_string")?;

    // Construct a synthetic unified diff from the string-replace pair.
    // ParsedDiff is the canonical type for file edits in Warp. We build
    // one that represents "replace old_string with new_string in file_path".
    let diff = build_search_replace_diff(file_path, old_string, new_string);

    Ok(AIAgentActionType::RequestFileEdits {
        file_edits: vec![FileEdit::Edit(diff)],
        title: None,
    })
}

// ── write (create/overwrite) ─────────────────────────────────────────

fn parse_write(input: &Value) -> Result<AIAgentActionType, AdapterError> {
    let file_path = require_str(input, "write", "file_path")?;
    let content = require_str(input, "write", "content")?;

    Ok(AIAgentActionType::RequestFileEdits {
        file_edits: vec![FileEdit::Create {
            file: Some(file_path.to_owned()),
            content: Some(content.to_owned()),
        }],
        title: None,
    })
}

// ── patch (unified diff) ─────────────────────────────────────────────

fn parse_patch(input: &Value) -> Result<AIAgentActionType, AdapterError> {
    let file_path = require_str(input, "patch", "file_path")?;
    let diff_text = require_str(input, "patch", "diff")?;

    let diff = build_unified_diff(file_path, diff_text);

    Ok(AIAgentActionType::RequestFileEdits {
        file_edits: vec![FileEdit::Edit(diff)],
        title: None,
    })
}

// ── glob ─────────────────────────────────────────────────────────────

fn parse_glob(input: &Value) -> Result<AIAgentActionType, AdapterError> {
    let pattern = require_str(input, "glob", "pattern")?;
    let path = input.get("path").and_then(Value::as_str).map(String::from);

    Ok(AIAgentActionType::FileGlob {
        patterns: vec![pattern.to_owned()],
        path,
    })
}

// ── grep ─────────────────────────────────────────────────────────────

fn parse_grep(input: &Value) -> Result<AIAgentActionType, AdapterError> {
    let pattern = require_str(input, "grep", "pattern")?;
    let path = input
        .get("path")
        .and_then(Value::as_str)
        .unwrap_or(".")
        .to_owned();

    Ok(AIAgentActionType::Grep {
        queries: vec![pattern.to_owned()],
        path,
    })
}

// ── Helpers ──────────────────────────────────────────────────────────

fn require_str<'a>(
    input: &'a Value,
    tool: &'static str,
    field: &'static str,
) -> Result<&'a str, AdapterError> {
    input
        .get(field)
        .and_then(Value::as_str)
        .ok_or(AdapterError::MissingField { tool, field })
}

/// Build a `ParsedDiff` representing a search-replace operation.
///
/// Constructs `ParsedDiff::StrReplaceEdit` — the canonical type for
/// string-replacement edits in Warp's diff pipeline. Line-number
/// resolution happens downstream in the block executor.
fn build_search_replace_diff(file_path: &str, old_string: &str, new_string: &str) -> ParsedDiff {
    ParsedDiff::StrReplaceEdit {
        file: Some(file_path.to_owned()),
        search: Some(old_string.to_owned()),
        replace: Some(new_string.to_owned()),
    }
}

/// Build a `ParsedDiff` from a raw diff string produced by opencode's
/// `patch` tool.
///
/// Phase 1 stub: wraps the entire diff as a single V4A hunk with the
/// raw text in `old`. The executor will fail gracefully ("no match")
/// since proper hunk parsing isn't wired yet. Phase 2 replaces this
/// with a real parser that splits hunks on `@@` markers (bd: warp-317.2.6).
fn build_unified_diff(file_path: &str, diff_text: &str) -> ParsedDiff {
    ParsedDiff::V4AEdit {
        file: Some(file_path.to_owned()),
        move_to: None,
        hunks: vec![V4AHunk {
            change_context: Vec::new(),
            pre_context: String::new(),
            old: diff_text.to_owned(),
            new: String::new(),
            post_context: String::new(),
        }],
    }
}

#[cfg(test)]
#[path = "action_adapter_tests.rs"]
mod action_adapter_tests;
