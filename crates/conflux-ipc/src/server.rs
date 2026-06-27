use std::sync::Arc;
use std::time::Instant;

use conflux_core::{fetch_and_normalize, ConfluxSubscription};
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

use crate::protocol::{
    parse_request_line, ProtocolError, Request, RequestCommand, Response, PROTOCOL_VERSION,
};

/// Shared daemon state exposed through IPC.
#[derive(Debug)]
pub struct EngineState {
    pub profile: Arc<RwLock<Option<ConfluxSubscription>>>,
    pub started_at: Instant,
    pub last_error: Arc<RwLock<Option<String>>>,
    pub last_fetch_url: Arc<RwLock<Option<String>>>,
}

impl Default for EngineState {
    fn default() -> Self {
        Self::new()
    }
}

impl EngineState {
    pub fn new() -> Self {
        Self {
            profile: Arc::new(RwLock::new(None)),
            started_at: Instant::now(),
            last_error: Arc::new(RwLock::new(None)),
            last_fetch_url: Arc::new(RwLock::new(None)),
        }
    }

    pub async fn status_json(&self) -> Value {
        let profile = self.profile.read().await;
        let last_fetch_url = self.last_fetch_url.read().await.clone();
        json!({
            "version": env!("CARGO_PKG_VERSION"),
            "protocol_version": PROTOCOL_VERSION,
            "uptime_secs": self.started_at.elapsed().as_secs(),
            "has_profile": profile.is_some(),
            "node_count": profile.as_ref().map(|p| p.nodes.len()).unwrap_or(0),
            "title": profile.as_ref().map(|p| p.title.clone()),
            "last_fetch_url": conflux_protocol::redact_url_for_ipc(&last_fetch_url),
            "last_error": self.last_error.read().await.clone(),
        })
    }
}

pub struct IpcServer {
    endpoint: String,
    state: Arc<EngineState>,
}

impl IpcServer {
    pub fn new(endpoint: impl Into<String>) -> Self {
        Self {
            endpoint: endpoint.into(),
            state: Arc::new(EngineState::new()),
        }
    }

    pub fn with_state(endpoint: impl Into<String>, state: Arc<EngineState>) -> Self {
        Self {
            endpoint: endpoint.into(),
            state,
        }
    }

    pub fn state(&self) -> Arc<EngineState> {
        Arc::clone(&self.state)
    }

    pub async fn run(self) -> Result<(), ProtocolError> {
        info!(endpoint = %self.endpoint, "starting IPC server");
        #[cfg(windows)]
        {
            self.run_windows().await
        }
        #[cfg(not(windows))]
        {
            self.run_unix().await
        }
    }

    #[cfg(windows)]
    async fn run_windows(&self) -> Result<(), ProtocolError> {
        use tokio::net::windows::named_pipe::ServerOptions;

        let mut server = ServerOptions::new()
            .first_pipe_instance(true)
            .create(&self.endpoint)
            .map_err(|err| ProtocolError::Transport(err.to_string()))?;

        loop {
            server
                .connect()
                .await
                .map_err(|err| ProtocolError::Transport(err.to_string()))?;

            let connected = server;
            server = ServerOptions::new()
                .create(&self.endpoint)
                .map_err(|err| ProtocolError::Transport(err.to_string()))?;

            let state = Arc::clone(&self.state);
            tokio::spawn(async move {
                if let Err(err) = serve_connection(connected, state).await {
                    warn!(error = %err, "IPC client session ended with error");
                }
            });
        }
    }

    #[cfg(not(windows))]
    async fn run_unix(&self) -> Result<(), ProtocolError> {
        use tokio::net::UnixListener;

        let _ = std::fs::remove_file(&self.endpoint);
        let listener = UnixListener::bind(&self.endpoint)
            .map_err(|err| ProtocolError::Transport(err.to_string()))?;

        loop {
            let (stream, _) = listener
                .accept()
                .await
                .map_err(|err| ProtocolError::Transport(err.to_string()))?;
            let state = Arc::clone(&self.state);
            tokio::spawn(async move {
                if let Err(err) = serve_connection(stream, state).await {
                    warn!(error = %err, "IPC client session ended with error");
                }
            });
        }
    }
}

async fn serve_connection(
    stream: impl tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + 'static,
    state: Arc<EngineState>,
) -> Result<(), ProtocolError> {
    let (reader, mut writer) = tokio::io::split(stream);
    let mut lines = BufReader::new(reader).lines();

    while let Some(line) = lines
        .next_line()
        .await
        .map_err(|err| ProtocolError::Transport(err.to_string()))?
    {
        let response = match parse_request_line(&line) {
            Ok(request) => handle_request(request, &state).await,
            Err(err) => Response::err(err.to_string()),
        };

        let payload = response.to_line()?;
        writer
            .write_all(payload.as_bytes())
            .await
            .map_err(|err| ProtocolError::Transport(err.to_string()))?;
        writer
            .write_all(b"\n")
            .await
            .map_err(|err| ProtocolError::Transport(err.to_string()))?;
        writer
            .flush()
            .await
            .map_err(|err| ProtocolError::Transport(err.to_string()))?;
    }

    Ok(())
}

async fn handle_request(request: Request, state: &EngineState) -> Response {
    debug!(?request.cmd, "IPC request");

    match request.cmd {
        RequestCommand::Ping => Response::ok(json!({
            "pong": true,
            "version": PROTOCOL_VERSION,
            "engine": env!("CARGO_PKG_VERSION"),
        })),
        RequestCommand::Status => Response::ok(state.status_json().await),
        RequestCommand::GetProfile => match state.profile.read().await.clone() {
            Some(profile) => match serde_json::to_value(profile.redacted_for_ipc()) {
                Ok(value) => Response::ok(value),
                Err(err) => Response::err(format!("failed to serialize profile: {err}")),
            },
            None => Response::err("no profile loaded"),
        },
        RequestCommand::Fetch => {
            let url = request.url.expect("validated by parse_request_line");
            match fetch_and_normalize(&url).await {
                Ok(profile) => {
                    *state.last_fetch_url.write().await = Some(url);
                    *state.last_error.write().await = None;
                    let summary = profile.fetch_summary();
                    *state.profile.write().await = Some(profile);
                    Response::ok(summary)
                }
                Err(err) => {
                    let message = err.to_string();
                    error!(error = %message, "fetch failed");
                    *state.last_error.write().await = Some(message.clone());
                    Response::err(message)
                }
            }
        }
    }
}

pub async fn exchange(
    stream: impl tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
    request: &Request,
) -> Result<Response, ProtocolError> {
    let (reader, mut writer) = tokio::io::split(stream);
    let line =
        serde_json::to_string(request).map_err(|err| ProtocolError::Serialize(err.to_string()))?;
    writer
        .write_all(line.as_bytes())
        .await
        .map_err(|err| ProtocolError::Transport(err.to_string()))?;
    writer
        .write_all(b"\n")
        .await
        .map_err(|err| ProtocolError::Transport(err.to_string()))?;
    writer
        .flush()
        .await
        .map_err(|err| ProtocolError::Transport(err.to_string()))?;

    let mut response_line = String::new();
    let mut lines = BufReader::new(reader).lines();
    if let Some(line) = lines
        .next_line()
        .await
        .map_err(|err| ProtocolError::Transport(err.to_string()))?
    {
        response_line = line;
    }

    if response_line.trim().is_empty() {
        return Err(ProtocolError::Transport("empty response".into()));
    }

    serde_json::from_str(&response_line)
        .map_err(|err| ProtocolError::InvalidRequest(err.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::{Request, ResponseStatus};

    #[tokio::test]
    async fn handles_ping_in_memory() {
        let state = Arc::new(EngineState::new());
        let response = handle_request(Request::ping(), &state).await;
        assert_eq!(response.status, ResponseStatus::Ok);
    }
}
