use std::sync::Arc;

use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;

use crate::rpc_server::{error_codes, Handler, HandlerResult, RpcError, RpcServer};

/// Test handler that echoes params back on "echo", returns error on "fail",
/// and METHOD_NOT_FOUND for anything else.
struct TestHandler;

#[async_trait::async_trait]
impl Handler for TestHandler {
    async fn handle_request(&self, method: &str, params: Value) -> HandlerResult {
        match method {
            "echo" => Ok(params),
            "fail" => Err(RpcError {
                code: -1,
                message: "intentional failure".to_string(),
                data: Some(json!({"detail": "test"})),
            }),
            _ => Err(RpcError {
                code: error_codes::METHOD_NOT_FOUND,
                message: format!("unknown method: {method}"),
                data: None,
            }),
        }
    }
}

/// Connect to the server and return a line-buffered reader + writer.
async fn connect(
    server: &RpcServer,
) -> (
    BufReader<tokio::net::tcp::OwnedReadHalf>,
    tokio::net::tcp::OwnedWriteHalf,
) {
    let stream = TcpStream::connect(server.addr()).await.unwrap();
    let (r, w) = stream.into_split();
    (BufReader::new(r), w)
}

/// Send a JSON-RPC request and read the response line.
async fn send_and_recv(
    writer: &mut tokio::net::tcp::OwnedWriteHalf,
    reader: &mut BufReader<tokio::net::tcp::OwnedReadHalf>,
    msg: &Value,
) -> Value {
    let mut buf = serde_json::to_vec(msg).unwrap();
    buf.push(b'\n');
    writer.write_all(&buf).await.unwrap();

    let mut line = String::new();
    reader.read_line(&mut line).await.unwrap();
    serde_json::from_str(&line).unwrap()
}

#[tokio::test]
async fn request_echo_returns_params() {
    let server = RpcServer::start(Arc::new(TestHandler)).await.unwrap();
    let (mut reader, mut writer) = connect(&server).await;

    let req = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "echo",
        "params": {"hello": "world"}
    });

    let resp = send_and_recv(&mut writer, &mut reader, &req).await;
    assert_eq!(resp["jsonrpc"], "2.0");
    assert_eq!(resp["id"], 1);
    assert_eq!(resp["result"], json!({"hello": "world"}));
    assert!(resp.get("error").is_none());

    server.shutdown().await;
}

#[tokio::test]
async fn request_error_returns_rpc_error() {
    let server = RpcServer::start(Arc::new(TestHandler)).await.unwrap();
    let (mut reader, mut writer) = connect(&server).await;

    let req = json!({
        "jsonrpc": "2.0",
        "id": 42,
        "method": "fail",
        "params": null
    });

    let resp = send_and_recv(&mut writer, &mut reader, &req).await;
    assert_eq!(resp["id"], 42);
    assert_eq!(resp["error"]["code"], -1);
    assert_eq!(resp["error"]["message"], "intentional failure");
    assert_eq!(resp["error"]["data"]["detail"], "test");
    assert!(resp.get("result").is_none());

    server.shutdown().await;
}

#[tokio::test]
async fn unknown_method_returns_method_not_found() {
    let server = RpcServer::start(Arc::new(TestHandler)).await.unwrap();
    let (mut reader, mut writer) = connect(&server).await;

    let req = json!({
        "jsonrpc": "2.0",
        "id": 7,
        "method": "nonexistent"
    });

    let resp = send_and_recv(&mut writer, &mut reader, &req).await;
    assert_eq!(resp["error"]["code"], error_codes::METHOD_NOT_FOUND);

    server.shutdown().await;
}

#[tokio::test]
async fn notification_gets_no_response() {
    let server = RpcServer::start(Arc::new(TestHandler)).await.unwrap();
    let (mut reader, mut writer) = connect(&server).await;

    // Send notification (no id)
    let notif = json!({
        "jsonrpc": "2.0",
        "method": "some.event",
        "params": {"data": 1}
    });
    let mut buf = serde_json::to_vec(&notif).unwrap();
    buf.push(b'\n');
    writer.write_all(&buf).await.unwrap();

    // Then send a request to verify the connection is still alive
    let req = json!({
        "jsonrpc": "2.0",
        "id": 99,
        "method": "echo",
        "params": "after_notif"
    });
    let resp = send_and_recv(&mut writer, &mut reader, &req).await;
    assert_eq!(resp["id"], 99);
    assert_eq!(resp["result"], "after_notif");

    server.shutdown().await;
}

#[tokio::test]
async fn malformed_json_returns_parse_error() {
    let server = RpcServer::start(Arc::new(TestHandler)).await.unwrap();
    let (mut reader, mut writer) = connect(&server).await;

    // Send invalid JSON
    writer.write_all(b"this is not json\n").await.unwrap();

    let mut line = String::new();
    reader.read_line(&mut line).await.unwrap();
    let resp: Value = serde_json::from_str(&line).unwrap();
    assert_eq!(resp["error"]["code"], error_codes::PARSE_ERROR);

    server.shutdown().await;
}

#[tokio::test]
async fn string_id_supported() {
    let server = RpcServer::start(Arc::new(TestHandler)).await.unwrap();
    let (mut reader, mut writer) = connect(&server).await;

    let req = json!({
        "jsonrpc": "2.0",
        "id": "abc-123",
        "method": "echo",
        "params": 42
    });

    let resp = send_and_recv(&mut writer, &mut reader, &req).await;
    assert_eq!(resp["id"], "abc-123");
    assert_eq!(resp["result"], 42);

    server.shutdown().await;
}

#[tokio::test]
async fn shutdown_stops_server() {
    let server = RpcServer::start(Arc::new(TestHandler)).await.unwrap();
    let addr = server.addr();

    server.shutdown().await;

    // Connection should be refused after shutdown
    let result = TcpStream::connect(addr).await;
    assert!(result.is_err());
}
