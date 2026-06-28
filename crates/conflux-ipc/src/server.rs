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

#[derive(Debug, Default)]
struct EngineStateData {
    profile: Option<ConfluxSubscription>,
    last_error: Option<String>,
    last_fetch_url: Option<String>,
}

/// Shared daemon state exposed through IPC.
#[derive(Debug)]
pub struct EngineState {
    pub started_at: Instant,
    data: Arc<RwLock<EngineStateData>>,
}

impl Default for EngineState {
    fn default() -> Self {
        Self::new()
    }
}

impl EngineState {
    pub fn new() -> Self {
        Self {
            started_at: Instant::now(),
            data: Arc::new(RwLock::new(EngineStateData::default())),
        }
    }

    pub async fn status_json(&self) -> Value {
        let data = self.data.read().await;
        json!({
            "version": env!("CARGO_PKG_VERSION"),
            "protocol_version": PROTOCOL_VERSION,
            "uptime_secs": self.started_at.elapsed().as_secs(),
            "has_profile": data.profile.is_some(),
            "node_count": data.profile.as_ref().map(|p| p.nodes.len()).unwrap_or(0),
            "title": data.profile.as_ref().map(|p| p.title.clone()),
            "last_fetch_url": data.last_fetch_url.clone(),
            "last_error": data.last_error.clone(),
        })
    }

    pub async fn set_profile(&self, url: String, profile: ConfluxSubscription) {
        let mut data = self.data.write().await;
        data.last_fetch_url = Some(url);
        data.last_error = None;
        data.profile = Some(profile);
    }

    pub async fn set_fetch_error(&self, message: String) {
        let mut data = self.data.write().await;
        data.last_error = Some(message);
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
        RequestCommand::GetProfile => {
            let profile = state.data.read().await.profile.clone();
            match profile {
                Some(profile) => match serde_json::to_value(profile.redacted_for_ipc()) {
                    Ok(value) => Response::ok(value),
                    Err(err) => Response::err(format!("failed to serialize profile: {err}")),
                },
                None => Response::err("no profile loaded"),
            }
        }
        RequestCommand::Fetch => {
            let url = request.url.expect("validated by parse_request_line");
            match fetch_and_normalize(&url).await {
                Ok(profile) => {
                    let summary = profile.fetch_summary();
                    state.set_profile(url, profile).await;
                    Response::ok(summary)
                }
                Err(err) => {
                    let message = err.to_string();
                    error!(error = %message, "fetch failed");
                    state.set_fetch_error(message.clone()).await;
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
    use conflux_protocol::{ConfluxNode, ConfluxSubscription, Protocol};

    #[tokio::test]
    async fn handles_ping_in_memory() {
        let state = Arc::new(EngineState::new());
        let response = handle_request(Request::ping(), &state).await;
        assert_eq!(response.status, ResponseStatus::Ok);
    }

    #[tokio::test]
    async fn status_reflects_atomic_profile_update() {
        let state = EngineState::new();
        let profile = ConfluxSubscription {
            title: "Test".to_string(),
            source_url: None,
            update_interval_hours: 0,
            user_info: None,
            support_url: None,
            announce: None,
            nodes: vec![ConfluxNode {
                id: "node-1".to_string(),
                tag: "node".to_string(),
                protocol: Protocol::Vless,
                source: Default::default(),
                server: "example.com".to_string(),
                port: 443,
                ports: None,
                credentials: conflux_protocol::Credentials::None,
                transport: Default::default(),
                tls: None,
                reality: None,
                flow: None,
                encryption: None,
                packet_encoding: None,
                method: None,
                obfs: None,
                meta: Default::default(),
                raw: conflux_protocol::RawPayload::Uri {
                    value: "vless://example".to_string(),
                },
                native_profile: None,
                native_tun_cidr: None,
                usage_url: None,
            }],
            extras: Default::default(),
        };

        state
            .set_profile("https://example.com/sub".to_string(), profile)
            .await;

        let status = state.status_json().await;
        assert_eq!(status["node_count"], 1);
        assert_eq!(status["title"], "Test");
        assert_eq!(status["last_fetch_url"], "https://example.com/sub");
    }
}
