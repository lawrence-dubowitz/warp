use std::sync::Once;
use std::time::Duration;

use mockito::Server;

use crate::process_supervisor::HttpEndpoint;
use crate::sse_demuxer::SseDemuxer;

static CRYPTO_INIT: Once = Once::new();

/// Workspace `reqwest` is built with `rustls-tls-native-roots-no-provider` so
/// every test that constructs a `reqwest::Client` (including transitively via
/// `reqwest_eventsource::RequestBuilderExt::eventsource`) must install a
/// crypto provider once per process. Production install site is
/// `app::run()` in `app/src/lib.rs`. The result is intentionally discarded
/// because nextest may share a process across tests.
fn install_test_crypto_provider() {
    CRYPTO_INIT.call_once(|| {
        let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();
    });
}

fn endpoint(server: &Server) -> HttpEndpoint {
    HttpEndpoint {
        base_url: server.url(),
    }
}

fn sse_body(frames: &[&str]) -> String {
    let mut body = String::new();
    for frame in frames {
        body.push_str("data: ");
        body.push_str(frame);
        body.push_str("\n\n");
    }
    body
}

#[tokio::test]
async fn subscribe_routes_event_to_matching_session() {
    install_test_crypto_provider();
    let mut server = Server::new_async().await;
    let body = sse_body(&[
        r#"{"type":"server.connected","properties":{}}"#,
        r#"{"type":"message.part.delta","properties":{"sessionID":"ses_abc","delta":"hi"}}"#,
    ]);
    let _mock = server
        .mock("GET", "/event")
        .with_status(200)
        .with_header("content-type", "text/event-stream")
        .with_header("cache-control", "no-cache")
        .with_body(body)
        .create_async()
        .await;

    let demuxer = SseDemuxer::new(endpoint(&server));
    let rx = demuxer.subscribe("ses_abc").await;

    let received = tokio::time::timeout(Duration::from_secs(2), rx.recv())
        .await
        .expect("event received before timeout")
        .expect("channel still open");
    assert_eq!(received.r#type, "message.part.delta");
    assert_eq!(received.session_id(), Some("ses_abc"));

    demuxer.shutdown().await;
}

#[tokio::test]
async fn events_for_unregistered_session_are_dropped() {
    install_test_crypto_provider();
    let mut server = Server::new_async().await;
    let body = sse_body(&[
        r#"{"type":"message.part.delta","properties":{"sessionID":"ses_other","delta":"x"}}"#,
        r#"{"type":"message.part.delta","properties":{"sessionID":"ses_target","delta":"y"}}"#,
    ]);
    let _mock = server
        .mock("GET", "/event")
        .with_status(200)
        .with_header("content-type", "text/event-stream")
        .with_body(body)
        .create_async()
        .await;

    let demuxer = SseDemuxer::new(endpoint(&server));
    let rx = demuxer.subscribe("ses_target").await;

    let received = tokio::time::timeout(Duration::from_secs(2), rx.recv())
        .await
        .expect("event received before timeout")
        .expect("channel still open");
    assert_eq!(received.session_id(), Some("ses_target"));
    assert!(rx.try_recv().is_err());

    demuxer.shutdown().await;
}

#[tokio::test]
async fn non_session_events_are_dropped() {
    install_test_crypto_provider();
    let mut server = Server::new_async().await;
    let body = sse_body(&[
        r#"{"type":"server.heartbeat","properties":{}}"#,
        r#"{"type":"server.connected","properties":{}}"#,
        r#"{"type":"message.part.delta","properties":{"sessionID":"ses_abc","delta":"hi"}}"#,
    ]);
    let _mock = server
        .mock("GET", "/event")
        .with_status(200)
        .with_header("content-type", "text/event-stream")
        .with_body(body)
        .create_async()
        .await;

    let demuxer = SseDemuxer::new(endpoint(&server));
    let rx = demuxer.subscribe("ses_abc").await;

    let received = tokio::time::timeout(Duration::from_secs(2), rx.recv())
        .await
        .expect("event received before timeout")
        .expect("channel still open");
    assert_eq!(received.r#type, "message.part.delta");

    demuxer.shutdown().await;
}

#[tokio::test]
async fn unsubscribe_closes_receiver() {
    install_test_crypto_provider();
    let mut server = Server::new_async().await;
    let _mock = server
        .mock("GET", "/event")
        .with_status(200)
        .with_header("content-type", "text/event-stream")
        .with_body(String::new())
        .expect_at_least(1)
        .create_async()
        .await;

    let demuxer = SseDemuxer::new(endpoint(&server));
    let rx = demuxer.subscribe("ses_abc").await;
    demuxer.unsubscribe("ses_abc");

    let result = tokio::time::timeout(Duration::from_secs(1), rx.recv()).await;
    let recv = result.expect("recv resolved before timeout");
    assert!(recv.is_err(), "channel should be closed after unsubscribe");

    demuxer.shutdown().await;
}

#[tokio::test]
async fn shutdown_stops_producer_and_closes_subscribers() {
    install_test_crypto_provider();
    let mut server = Server::new_async().await;
    let _mock = server
        .mock("GET", "/event")
        .with_status(200)
        .with_header("content-type", "text/event-stream")
        .with_body(String::new())
        .expect_at_least(1)
        .create_async()
        .await;

    let demuxer = SseDemuxer::new(endpoint(&server));
    let rx = demuxer.subscribe("ses_abc").await;

    demuxer.shutdown().await;

    let result = tokio::time::timeout(Duration::from_secs(1), rx.recv()).await;
    let recv = result.expect("recv resolved before timeout");
    assert!(recv.is_err(), "channel should be closed after shutdown");
}
