use serde_json::json;

use super::*;

#[test]
fn is_allowed_accepts_phase_1_tools() {
    for tool in ALLOWED_TOOLS {
        assert!(is_allowed(tool), "{tool} should be allowed");
    }
}

#[test]
fn is_allowed_rejects_disabled_tools() {
    for tool in &[
        "webfetch", "todo", "task", "fetch", "search", "code", "skill", "lsp", "plan",
    ] {
        assert!(!is_allowed(tool), "{tool} should be rejected");
    }
}

#[test]
fn parse_bash_extracts_command() {
    let input = json!({"command": "ls -la"});
    let action = parse_action("bash", &input).unwrap();
    match action {
        AIAgentActionType::RequestCommandOutput { command, .. } => {
            assert_eq!(command, "ls -la");
        }
        other => panic!("expected RequestCommandOutput, got {other:?}"),
    }
}

#[test]
fn parse_bash_missing_command_errors() {
    let input = json!({});
    let err = parse_action("bash", &input).unwrap_err();
    assert!(matches!(
        err,
        AdapterError::MissingField {
            tool: "bash",
            field: "command"
        }
    ));
}

#[test]
fn parse_read_file_path_only() {
    let input = json!({"file_path": "/tmp/foo.rs"});
    let action = parse_action("read", &input).unwrap();
    match action {
        AIAgentActionType::ReadFiles(req) => {
            assert_eq!(req.locations.len(), 1);
            assert_eq!(req.locations[0].name, "/tmp/foo.rs");
            assert!(req.locations[0].lines.is_empty());
        }
        other => panic!("expected ReadFiles, got {other:?}"),
    }
}

#[test]
fn parse_read_with_offset_and_limit() {
    let input = json!({"file_path": "src/lib.rs", "offset": 10, "limit": 50});
    let action = parse_action("read", &input).unwrap();
    match action {
        AIAgentActionType::ReadFiles(req) => {
            assert_eq!(req.locations[0].lines, vec![10..60]);
        }
        other => panic!("expected ReadFiles, got {other:?}"),
    }
}

#[test]
fn parse_edit_constructs_str_replace() {
    let input = json!({"file_path": "foo.rs", "old_string": "hello", "new_string": "world"});
    let action = parse_action("edit", &input).unwrap();
    match action {
        AIAgentActionType::RequestFileEdits { file_edits, .. } => {
            assert_eq!(file_edits.len(), 1);
            match &file_edits[0] {
                FileEdit::Edit(diff) => {
                    assert_eq!(diff.file(), Some(&"foo.rs".to_owned()));
                }
                other => panic!("expected FileEdit::Edit, got {other:?}"),
            }
        }
        other => panic!("expected RequestFileEdits, got {other:?}"),
    }
}

#[test]
fn parse_write_constructs_create() {
    let input = json!({"file_path": "new.rs", "content": "fn main() {}"});
    let action = parse_action("write", &input).unwrap();
    match action {
        AIAgentActionType::RequestFileEdits { file_edits, .. } => {
            assert_eq!(file_edits.len(), 1);
            match &file_edits[0] {
                FileEdit::Create { file, content } => {
                    assert_eq!(file.as_deref(), Some("new.rs"));
                    assert_eq!(content.as_deref(), Some("fn main() {}"));
                }
                other => panic!("expected FileEdit::Create, got {other:?}"),
            }
        }
        other => panic!("expected RequestFileEdits, got {other:?}"),
    }
}

#[test]
fn parse_glob_with_optional_path() {
    let input = json!({"pattern": "**/*.rs", "path": "src/"});
    let action = parse_action("glob", &input).unwrap();
    match action {
        AIAgentActionType::FileGlob { patterns, path } => {
            assert_eq!(patterns, vec!["**/*.rs"]);
            assert_eq!(path, Some("src/".to_owned()));
        }
        other => panic!("expected FileGlob, got {other:?}"),
    }
}

#[test]
fn parse_grep_defaults_path_to_dot() {
    let input = json!({"pattern": "TODO"});
    let action = parse_action("grep", &input).unwrap();
    match action {
        AIAgentActionType::Grep { queries, path } => {
            assert_eq!(queries, vec!["TODO"]);
            assert_eq!(path, ".");
        }
        other => panic!("expected Grep, got {other:?}"),
    }
}

#[test]
fn disabled_tool_returns_error() {
    let input = json!({});
    let err = parse_action("webfetch", &input).unwrap_err();
    assert!(matches!(err, AdapterError::DisabledTool(name) if name == "webfetch"));
}
