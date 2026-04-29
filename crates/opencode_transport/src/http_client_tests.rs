use std::sync::Once;

use mockito::Server;
use reqwest::StatusCode;

use super::{OpencodeApiError, OpencodeClient};
use crate::schemas::{CreateSessionRequest, PromptAsyncRequest, PromptPart};

static CRYPTO_INIT: Once = Once::new();

// Workspace `reqwest` is configured with `rustls-tls-native-roots-no-provider`,
// so any `reqwest::Client` constructor panics with "No provider set" until the
// process installs a default crypto provider. Production wiring lives in
// `app::run()` (`app/src/lib.rs`); tests must opt in explicitly.
fn install_test_crypto_provider() {
    CRYPTO_INIT.call_once(|| {
        let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();
    });
}

fn make_client(server: &Server) -> OpencodeClient {
    install_test_crypto_provider();
    OpencodeClient::new(server.url())
}

#[tokio::test]
async fn create_session_returns_decoded_session_info() {
    let mut server = Server::new_async().await;
    let mock = server
        .mock("POST", "/session")
        .match_header("content-type", "application/json")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            r#"{
                "id": "ses_abc",
                "title": "scratch",
                "projectID": "proj_xyz",
                "workspaceID": "ws_1",
                "directory": "/tmp",
                "extra_field_we_dont_know": true
            }"#,
        )
        .create_async()
        .await;

    let client = make_client(&server);
    let info = client
        .create_session(&CreateSessionRequest::default())
        .await
        .expect("create_session ok");

    assert_eq!(info.id, "ses_abc");
    assert_eq!(info.title.as_deref(), Some("scratch"));
    assert_eq!(info.project_id.as_deref(), Some("proj_xyz"));
    assert_eq!(info.workspace_id.as_deref(), Some("ws_1"));
    assert_eq!(info.directory.as_deref(), Some("/tmp"));
    mock.assert_async().await;
}

#[tokio::test]
async fn get_session_returns_decoded_session_info() {
    let mut server = Server::new_async().await;
    let mock = server
        .mock("GET", "/session/ses_abc")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"id": "ses_abc"}"#)
        .create_async()
        .await;

    let client = make_client(&server);
    let info = client.get_session("ses_abc").await.expect("get_session ok");

    assert_eq!(info.id, "ses_abc");
    assert!(info.title.is_none());
    mock.assert_async().await;
}

#[tokio::test]
async fn prompt_async_accepts_204_no_content() {
    let mut server = Server::new_async().await;
    let mock = server
        .mock("POST", "/session/ses_abc/prompt_async")
        .match_header("content-type", "application/json")
        .with_status(204)
        .create_async()
        .await;

    let client = make_client(&server);
    let request = PromptAsyncRequest {
        parts: vec![PromptPart::Text {
            text: "hello".to_string(),
        }],
        ..PromptAsyncRequest::default()
    };

    client
        .prompt_async("ses_abc", &request)
        .await
        .expect("prompt_async ok");
    mock.assert_async().await;
}

#[tokio::test]
async fn abort_accepts_200_with_body() {
    let mut server = Server::new_async().await;
    let mock = server
        .mock("POST", "/session/ses_abc/abort")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body("true")
        .create_async()
        .await;

    let client = make_client(&server);
    client.abort("ses_abc").await.expect("abort ok");
    mock.assert_async().await;
}

#[tokio::test]
async fn http_400_maps_to_http_variant_with_body() {
    let mut server = Server::new_async().await;
    let _mock = server
        .mock("POST", "/session")
        .with_status(400)
        .with_header("content-type", "application/json")
        .with_body(r#"{"success": false, "data": {}, "errors": [{"reason":"bad"}]}"#)
        .create_async()
        .await;

    let client = make_client(&server);
    let err = client
        .create_session(&CreateSessionRequest::default())
        .await
        .expect_err("expected http error");

    match err {
        OpencodeApiError::Http { status, body } => {
            assert_eq!(status, StatusCode::BAD_REQUEST);
            assert!(body.contains("\"success\""));
            assert!(body.contains("bad"));
        }
        other => panic!("expected Http variant, got {other:?}"),
    }
}
