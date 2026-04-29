use std::io::Write;
use std::path::PathBuf;
use std::sync::Once;
use std::time::Duration;

use tempfile::NamedTempFile;
use tokio::time::timeout;

use super::*;

static CRYPTO_INIT: Once = Once::new();

fn install_test_crypto_provider() {
    CRYPTO_INIT.call_once(|| {
        let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();
    });
}

fn write_unix_script(body: &str) -> PathBuf {
    let mut file = NamedTempFile::new().expect("create temp file");
    writeln!(file, "#!/bin/sh").unwrap();
    file.write_all(body.as_bytes()).unwrap();
    file.flush().unwrap();
    let path = file.into_temp_path().keep().expect("keep temp path");
    let mut perms = std::fs::metadata(&path).unwrap().permissions();
    use std::os::unix::fs::PermissionsExt;
    perms.set_mode(0o755);
    std::fs::set_permissions(&path, perms).unwrap();
    path
}

#[cfg(unix)]
#[tokio::test]
async fn ensure_running_returns_ready_timeout_for_silent_subprocess() {
    install_test_crypto_provider();
    let script = write_unix_script("sleep 60\n");
    let mut config = SupervisorConfig::new(script);
    config.host = "127.0.0.1".to_string();
    config.ready_probe_timeout = Duration::from_millis(500);
    config.max_spawn_attempts = 1;
    let supervisor = Supervisor::new(config);

    let outcome = timeout(Duration::from_secs(10), supervisor.ensure_running()).await;
    let result = outcome.expect("supervisor returned within outer timeout");
    match result {
        Err(SupervisorError::ReadyTimeout(_)) => {}
        Err(SupervisorError::Spawn { .. }) => {}
        other => panic!("expected ReadyTimeout or Spawn error, got {other:?}"),
    }
    supervisor.shutdown().await;
}

#[cfg(unix)]
#[tokio::test]
async fn shutdown_blocks_subsequent_ensure_running() {
    let script = write_unix_script("exit 0\n");
    let supervisor = Supervisor::new(SupervisorConfig::new(script));
    supervisor.shutdown().await;
    let result = supervisor.ensure_running().await;
    assert!(
        matches!(result, Err(SupervisorError::ShuttingDown)),
        "expected ShuttingDown after shutdown, got {result:?}"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn ensure_running_returns_spawn_error_for_missing_binary() {
    let supervisor = Supervisor::new(SupervisorConfig::new(PathBuf::from(
        "/nonexistent/opencode",
    )));
    let result = supervisor.ensure_running().await;
    assert!(
        matches!(result, Err(SupervisorError::Spawn { .. })),
        "expected Spawn error, got {result:?}"
    );
}

#[test]
fn pick_free_port_returns_distinct_ports() {
    let a = pick_free_port("127.0.0.1").expect("pick port a");
    let b = pick_free_port("127.0.0.1").expect("pick port b");
    assert_ne!(a, 0);
    assert_ne!(b, 0);
}
