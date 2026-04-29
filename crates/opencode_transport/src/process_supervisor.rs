//! Lazy-started, restart-on-failure supervisor for the `opencode serve` subprocess.
//!
//! One supervisor per Warp app. The subprocess is the single backend that all
//! conversations talk to over HTTP + SSE. See `specs/opencode-as-primary-agent/TECH.md`
//! and bd:warp-317.1.3.
//!
//! ## Invariants
//! - Subprocess is spawned via `command::r#async::Command` only. `std::process::Command`,
//!   `async_process::Command`, and `tokio::process::Command` are all forbidden in this
//!   workspace (`.clippy.toml`).
//! - `kill_on_drop(true)` is set so dropping the supervisor terminates the child.
//! - On Unix, the child is placed in its own process group so any descendants opencode
//!   spawns are also reaped on shutdown.
//! - Free port selection uses a one-shot `TcpListener::bind("127.0.0.1:0")` then drops
//!   the listener and passes the port to opencode. This races with anything else that
//!   could grab the port; in practice the window is microseconds and opencode binds
//!   almost immediately. If the bind races, the readiness probe surfaces it as a
//!   spawn failure and the next `ensure_running` retries.

use std::ffi::OsString;
use std::net::TcpListener;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use async_process::Child;
use command::r#async::Command;
use instant::Instant;
use tokio::sync::Mutex;
use tokio::time::sleep;

/// Maximum time we wait for opencode to start serving HTTP requests.
const READY_PROBE_TIMEOUT: Duration = Duration::from_secs(30);
/// Initial delay between readiness probes; doubles up to [`READY_PROBE_BACKOFF_MAX`].
const READY_PROBE_BACKOFF_INIT: Duration = Duration::from_millis(50);
/// Cap on the readiness-probe backoff.
const READY_PROBE_BACKOFF_MAX: Duration = Duration::from_secs(2);
/// Initial delay between restart attempts after a crash; doubles up to
/// [`RESTART_BACKOFF_MAX`]. Reset to [`RESTART_BACKOFF_INIT`] after the subprocess
/// stays healthy for [`RESTART_HEALTHY_THRESHOLD`].
const RESTART_BACKOFF_INIT: Duration = Duration::from_millis(100);
/// Cap on the restart backoff.
const RESTART_BACKOFF_MAX: Duration = Duration::from_secs(5);
/// How long the subprocess must stay up before we treat the next crash as fresh.
const RESTART_HEALTHY_THRESHOLD: Duration = Duration::from_secs(60);
/// How many spawn-and-probe attempts `ensure_running` will make in a single call
/// before giving up and returning the last error to the caller.
const MAX_SPAWN_ATTEMPTS: u32 = 5;

/// Configuration for a single opencode subprocess.
#[derive(Debug, Clone)]
pub struct SupervisorConfig {
    /// Absolute path to the `opencode` binary.
    pub opencode_bin: PathBuf,
    /// Working directory for the subprocess. If `None`, inherits Warp's cwd.
    pub working_dir: Option<PathBuf>,
    /// Extra environment variables passed to the subprocess on top of inherited env.
    pub extra_env: Vec<(OsString, OsString)>,
    /// HTTP host to bind opencode on. Defaults to `127.0.0.1`.
    pub host: String,
    /// How long to wait for opencode's HTTP endpoint to accept requests after
    /// spawn before giving up. Defaults to [`READY_PROBE_TIMEOUT`].
    pub ready_probe_timeout: Duration,
    /// Maximum number of spawn attempts inside a single `ensure_running` call
    /// before bubbling the last error to the caller. Defaults to
    /// [`MAX_SPAWN_ATTEMPTS`].
    pub max_spawn_attempts: u32,
}

impl SupervisorConfig {
    /// Create a config with sensible defaults: localhost bind, no extra env, inherits cwd.
    pub fn new(opencode_bin: PathBuf) -> Self {
        Self {
            opencode_bin,
            working_dir: None,
            extra_env: Vec::new(),
            host: "127.0.0.1".to_string(),
            ready_probe_timeout: READY_PROBE_TIMEOUT,
            max_spawn_attempts: MAX_SPAWN_ATTEMPTS,
        }
    }
}

/// Endpoint at which a running opencode instance is reachable.
#[derive(Debug, Clone)]
pub struct HttpEndpoint {
    /// Base URL with scheme, host, and port and no trailing slash, e.g.
    /// `http://127.0.0.1:54321`.
    pub base_url: String,
}

/// Errors the supervisor surfaces to its callers.
#[derive(Debug, thiserror::Error)]
pub enum SupervisorError {
    /// Failed to allocate a TCP port for opencode.
    #[error("failed to pick a free TCP port: {0}")]
    PortAllocation(#[source] std::io::Error),
    /// Failed to spawn the subprocess.
    #[error("failed to spawn opencode at {bin}: {source}")]
    Spawn {
        bin: PathBuf,
        #[source]
        source: std::io::Error,
    },
    /// Subprocess started but never accepted HTTP traffic within the timeout.
    #[error("opencode did not become ready within {0:?}")]
    ReadyTimeout(Duration),
    /// Supervisor was shut down while a caller was waiting on it.
    #[error("supervisor is shutting down")]
    ShuttingDown,
}

/// Lazy-started, restart-on-failure supervisor for `opencode serve`.
///
/// Cheap to clone — internally a single shared state behind an `Arc`.
#[derive(Clone)]
pub struct Supervisor {
    inner: Arc<SupervisorInner>,
}

struct SupervisorInner {
    config: SupervisorConfig,
    state: Mutex<State>,
}

enum State {
    NotStarted,
    Running {
        child: Child,
        endpoint: HttpEndpoint,
        started_at: Instant,
    },
    ShutDown,
}

impl Supervisor {
    /// Create a supervisor without spawning anything.
    pub fn new(config: SupervisorConfig) -> Self {
        Self {
            inner: Arc::new(SupervisorInner {
                config,
                state: Mutex::new(State::NotStarted),
            }),
        }
    }

    /// Idempotently ensure the subprocess is running and return its endpoint.
    ///
    /// On the first call, spawns opencode and waits for it to accept HTTP traffic.
    /// Subsequent calls return the cached endpoint until the subprocess exits, at
    /// which point the next call respawns with exponential backoff capped at
    /// [`RESTART_BACKOFF_MAX`].
    pub async fn ensure_running(&self) -> Result<HttpEndpoint, SupervisorError> {
        let mut backoff = RESTART_BACKOFF_INIT;
        let mut attempt: u32 = 0;
        loop {
            let mut state = self.inner.state.lock().await;
            match &mut *state {
                State::ShutDown => return Err(SupervisorError::ShuttingDown),
                State::Running {
                    child,
                    endpoint,
                    started_at,
                } => {
                    if let Ok(Some(status)) = child.try_status() {
                        tracing::warn!(
                            ?status,
                            "opencode subprocess exited; will respawn on next request"
                        );
                        if started_at.elapsed() >= RESTART_HEALTHY_THRESHOLD {
                            backoff = RESTART_BACKOFF_INIT;
                            attempt = 0;
                        }
                        *state = State::NotStarted;
                    } else {
                        return Ok(endpoint.clone());
                    }
                }
                State::NotStarted => {}
            }
            if attempt > 0 {
                tracing::info!(?backoff, attempt, "backing off before opencode restart");
                drop(state);
                sleep(backoff).await;
                backoff = (backoff * 2).min(RESTART_BACKOFF_MAX);
                let state_after_sleep = self.inner.state.lock().await;
                if matches!(&*state_after_sleep, State::ShutDown) {
                    return Err(SupervisorError::ShuttingDown);
                }
                drop(state_after_sleep);
            }
            attempt += 1;
            match self.spawn_and_wait_ready().await {
                Ok((child, endpoint)) => {
                    let mut state = self.inner.state.lock().await;
                    if matches!(&*state, State::ShutDown) {
                        let _ = child;
                        return Err(SupervisorError::ShuttingDown);
                    }
                    *state = State::Running {
                        child,
                        endpoint: endpoint.clone(),
                        started_at: Instant::now(),
                    };
                    return Ok(endpoint);
                }
                Err(err) => {
                    tracing::error!(?err, attempt, "failed to start opencode");
                    if attempt >= self.inner.config.max_spawn_attempts {
                        return Err(err);
                    }
                }
            }
        }
    }

    /// Stop the subprocess if running. After this returns, [`Self::ensure_running`]
    /// will always return [`SupervisorError::ShuttingDown`].
    pub async fn shutdown(&self) {
        let mut state = self.inner.state.lock().await;
        let prev = std::mem::replace(&mut *state, State::ShutDown);
        if let State::Running { mut child, .. } = prev {
            let pid = child.id();
            if let Err(err) = child.kill() {
                tracing::warn!(?err, pid, "failed to kill opencode child on shutdown");
            }
            tracing::info!(pid, "opencode subprocess shut down");
        }
    }

    async fn spawn_and_wait_ready(
        &self,
    ) -> Result<(Child, HttpEndpoint), SupervisorError> {
        let config = &self.inner.config;
        let port = pick_free_port(&config.host)?;
        let mut cmd = build_command(config, port);
        let child = cmd.spawn().map_err(|source| SupervisorError::Spawn {
            bin: config.opencode_bin.clone(),
            source,
        })?;
        let pid = child.id();
        let base_url = format!("http://{}:{}", config.host, port);
        tracing::info!(pid, %base_url, "spawned opencode subprocess");
        wait_until_ready(&base_url, config.ready_probe_timeout).await?;
        tracing::info!(pid, %base_url, "opencode subprocess is ready");
        Ok((child, HttpEndpoint { base_url }))
    }
}

fn build_command(config: &SupervisorConfig, port: u16) -> Command {
    let mut cmd = Command::new_with_process_group(&config.opencode_bin);
    cmd.arg("serve")
        .arg("--port")
        .arg(port.to_string())
        .arg("--hostname")
        .arg(&config.host)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .kill_on_drop(true);
    if let Some(dir) = &config.working_dir {
        cmd.current_dir(dir);
    }
    for (k, v) in &config.extra_env {
        cmd.env(k, v);
    }
    cmd
}

fn pick_free_port(host: &str) -> Result<u16, SupervisorError> {
    let listener =
        TcpListener::bind((host, 0)).map_err(SupervisorError::PortAllocation)?;
    let port = listener
        .local_addr()
        .map_err(SupervisorError::PortAllocation)?
        .port();
    drop(listener);
    Ok(port)
}

async fn wait_until_ready(
    base_url: &str,
    overall_timeout: Duration,
) -> Result<(), SupervisorError> {
    let probe_url = format!("{base_url}/app");
    let per_request_timeout = overall_timeout.min(Duration::from_secs(2));
    let client = reqwest::Client::builder()
        .timeout(per_request_timeout)
        .build()
        .map_err(|source| SupervisorError::Spawn {
            bin: PathBuf::from("<reqwest>"),
            source: std::io::Error::other(source),
        })?;
    let deadline = Instant::now() + overall_timeout;
    let mut delay = READY_PROBE_BACKOFF_INIT;
    loop {
        if Instant::now() >= deadline {
            return Err(SupervisorError::ReadyTimeout(overall_timeout));
        }
        match client.get(&probe_url).send().await {
            Ok(resp) if resp.status().is_success() => return Ok(()),
            Ok(resp) => {
                tracing::debug!(status = %resp.status(), "opencode probe non-success");
            }
            Err(err) => {
                tracing::debug!(?err, "opencode probe error");
            }
        }
        sleep(delay).await;
        delay = (delay * 2).min(READY_PROBE_BACKOFF_MAX);
    }
}

#[cfg(test)]
#[path = "process_supervisor_tests.rs"]
mod process_supervisor_tests;
