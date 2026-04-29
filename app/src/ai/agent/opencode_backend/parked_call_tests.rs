use serde_json::json;
use tokio::sync::oneshot;

use super::*;

fn make_call(id: &str) -> (ParkedToolCall, oneshot::Receiver<ToolResult>) {
    let (tx, rx) = oneshot::channel();
    let call = ParkedToolCall::new(
        id.to_owned(),
        "bash".to_owned(),
        json!({"command": "ls"}),
        tx,
    );
    (call, rx)
}

fn recv(rx: &mut oneshot::Receiver<ToolResult>) -> ToolResult {
    rx.try_recv().unwrap()
}

#[test]
fn new_call_starts_in_submitted() {
    let (call, _rx) = make_call("c1");
    assert_eq!(call.state(), ParkedToolCallState::Submitted);
    assert!(!call.state().is_terminal());
}

#[test]
fn happy_path_transitions() {
    let (mut call, mut rx) = make_call("c1");
    call.transition(ParkedToolCallState::AwaitingPreprocess)
        .unwrap();
    call.transition(ParkedToolCallState::AwaitingApproval)
        .unwrap();
    call.transition(ParkedToolCallState::Approved).unwrap();
    call.transition(ParkedToolCallState::Executing).unwrap();
    assert_eq!(call.state(), ParkedToolCallState::Executing);

    call.complete(ToolResult {
        output: Some(json!({"stdout": "hello"})),
        error: None,
    })
    .unwrap();
    assert_eq!(call.state(), ParkedToolCallState::Completed);
    assert!(call.state().is_terminal());

    let result = recv(&mut rx);
    assert_eq!(result.output, Some(json!({"stdout": "hello"})));
    assert!(result.error.is_none());
}

#[test]
fn fail_from_executing() {
    let (mut call, mut rx) = make_call("c1");
    call.transition(ParkedToolCallState::AwaitingPreprocess)
        .unwrap();
    call.transition(ParkedToolCallState::AwaitingApproval)
        .unwrap();
    call.transition(ParkedToolCallState::Approved).unwrap();
    call.transition(ParkedToolCallState::Executing).unwrap();
    call.fail("command not found".to_owned()).unwrap();

    assert_eq!(call.state(), ParkedToolCallState::Failed);
    let result = recv(&mut rx);
    assert_eq!(result.error.as_deref(), Some("command not found"));
    assert!(result.output.is_none());
}

#[test]
fn cancel_from_submitted() {
    let (mut call, mut rx) = make_call("c1");
    call.cancel().unwrap();
    assert_eq!(call.state(), ParkedToolCallState::Cancelled);
    let result = recv(&mut rx);
    assert_eq!(result.error.as_deref(), Some("cancelled"));
}

#[test]
fn cancel_from_awaiting_approval() {
    let (mut call, mut rx) = make_call("c1");
    call.transition(ParkedToolCallState::AwaitingPreprocess)
        .unwrap();
    call.transition(ParkedToolCallState::AwaitingApproval)
        .unwrap();
    call.cancel().unwrap();
    assert_eq!(call.state(), ParkedToolCallState::Cancelled);
    let result = recv(&mut rx);
    assert_eq!(result.error.as_deref(), Some("cancelled"));
}

#[test]
fn cancel_from_executing() {
    let (mut call, mut rx) = make_call("c1");
    call.transition(ParkedToolCallState::AwaitingPreprocess)
        .unwrap();
    call.transition(ParkedToolCallState::AwaitingApproval)
        .unwrap();
    call.transition(ParkedToolCallState::Approved).unwrap();
    call.transition(ParkedToolCallState::Executing).unwrap();
    call.cancel().unwrap();
    assert_eq!(call.state(), ParkedToolCallState::Cancelled);
    let result = recv(&mut rx);
    assert_eq!(result.error.as_deref(), Some("cancelled"));
}

#[test]
fn subprocess_lost_from_submitted() {
    let (mut call, mut rx) = make_call("c1");
    call.mark_subprocess_lost().unwrap();
    assert_eq!(call.state(), ParkedToolCallState::SubprocessLost);
    let result = recv(&mut rx);
    assert!(result.error.as_deref().unwrap().contains("restarted"));
}

#[test]
fn subprocess_lost_from_executing() {
    let (mut call, mut rx) = make_call("c1");
    call.transition(ParkedToolCallState::AwaitingPreprocess)
        .unwrap();
    call.transition(ParkedToolCallState::AwaitingApproval)
        .unwrap();
    call.transition(ParkedToolCallState::Approved).unwrap();
    call.transition(ParkedToolCallState::Executing).unwrap();
    call.mark_subprocess_lost().unwrap();
    assert_eq!(call.state(), ParkedToolCallState::SubprocessLost);
    let result = recv(&mut rx);
    assert!(result.error.as_deref().unwrap().contains("restarted"));
}

#[test]
fn invalid_transition_from_terminal() {
    let (mut call, _rx) = make_call("c1");
    call.cancel().unwrap();
    let err = call.transition(ParkedToolCallState::Executing).unwrap_err();
    assert_eq!(err.from, ParkedToolCallState::Cancelled);
    assert_eq!(err.to, ParkedToolCallState::Executing);
}

#[test]
fn invalid_transition_skip_states() {
    let (mut call, _rx) = make_call("c1");
    let err = call.transition(ParkedToolCallState::Executing).unwrap_err();
    assert_eq!(err.from, ParkedToolCallState::Submitted);
    assert_eq!(err.to, ParkedToolCallState::Executing);
}

#[test]
fn double_complete_returns_error() {
    let (mut call, _rx) = make_call("c1");
    call.transition(ParkedToolCallState::AwaitingPreprocess)
        .unwrap();
    call.transition(ParkedToolCallState::AwaitingApproval)
        .unwrap();
    call.transition(ParkedToolCallState::Approved).unwrap();
    call.transition(ParkedToolCallState::Executing).unwrap();
    call.complete(ToolResult {
        output: Some(json!("first")),
        error: None,
    })
    .unwrap();
    let err = call
        .complete(ToolResult {
            output: Some(json!("second")),
            error: None,
        })
        .unwrap_err();
    assert_eq!(err.from, ParkedToolCallState::Completed);
    assert_eq!(err.to, ParkedToolCallState::Completed);
}

#[test]
fn registry_insert_and_get() {
    let mut reg = ParkedCallRegistry::new();
    let (call, _rx) = make_call("c1");
    reg.insert(call);
    assert_eq!(reg.len(), 1);
    assert!(reg.get_mut("c1").is_some());
    assert!(reg.get_mut("c2").is_none());
}

#[test]
#[should_panic(expected = "duplicate opencode_call_id")]
fn registry_duplicate_insert_panics() {
    let mut reg = ParkedCallRegistry::new();
    let (call1, _rx1) = make_call("c1");
    let (call2, _rx2) = make_call("c1");
    reg.insert(call1);
    reg.insert(call2);
}

#[test]
fn registry_remove_if_terminal() {
    let mut reg = ParkedCallRegistry::new();
    let (call, _rx) = make_call("c1");
    reg.insert(call);

    assert!(reg.remove_if_terminal("c1").is_none());

    reg.get_mut("c1").unwrap().cancel().unwrap();
    let removed = reg.remove_if_terminal("c1").unwrap();
    assert_eq!(removed.state(), ParkedToolCallState::Cancelled);
    assert_eq!(reg.len(), 0);
}

#[test]
fn registry_check_sse_dedup_present() {
    let mut reg = ParkedCallRegistry::new();
    let (call, _rx) = make_call("c1");
    reg.insert(call);
    assert!(reg.check_sse_dedup("c1"));
}

#[test]
fn registry_check_sse_dedup_absent() {
    let reg = ParkedCallRegistry::new();
    assert!(!reg.check_sse_dedup("c_unknown"));
}

#[test]
fn registry_cancel_all() {
    let mut reg = ParkedCallRegistry::new();
    let (call1, _rx1) = make_call("c1");
    let (call2, _rx2) = make_call("c2");
    let (mut call3, _rx3) = make_call("c3");
    call3.cancel().unwrap();
    reg.insert(call1);
    reg.insert(call2);
    reg.insert(call3);

    let count = reg.cancel_all();
    assert_eq!(count, 2);
    assert_eq!(
        reg.get_mut("c1").unwrap().state(),
        ParkedToolCallState::Cancelled
    );
    assert_eq!(
        reg.get_mut("c2").unwrap().state(),
        ParkedToolCallState::Cancelled
    );
    assert_eq!(
        reg.get_mut("c3").unwrap().state(),
        ParkedToolCallState::Cancelled
    );
}

#[test]
fn registry_mark_all_subprocess_lost() {
    let mut reg = ParkedCallRegistry::new();
    let (call1, _rx1) = make_call("c1");
    let (mut call2, _rx2) = make_call("c2");
    call2
        .transition(ParkedToolCallState::AwaitingPreprocess)
        .unwrap();
    call2
        .transition(ParkedToolCallState::AwaitingApproval)
        .unwrap();
    call2.transition(ParkedToolCallState::Approved).unwrap();
    call2.transition(ParkedToolCallState::Executing).unwrap();
    reg.insert(call1);
    reg.insert(call2);

    let count = reg.mark_all_subprocess_lost();
    assert_eq!(count, 2);
    assert_eq!(
        reg.get_mut("c1").unwrap().state(),
        ParkedToolCallState::SubprocessLost
    );
    assert_eq!(
        reg.get_mut("c2").unwrap().state(),
        ParkedToolCallState::SubprocessLost
    );
}
