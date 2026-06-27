use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

pub const PROTOCOL_VERSION: u32 = 1;
pub const DEFAULT_PIPE_NAME: &str = r"\\.\pipe\conflux-engine";

#[cfg(unix)]
pub const DEFAULT_UNIX_SOCKET: &str = "/tmp/conflux-engine.sock";

/// IPC endpoint path for the current platform.
pub fn default_endpoint() -> String {
    #[cfg(windows)]
    {
        DEFAULT_PIPE_NAME.to_string()
    }
    #[cfg(not(windows))]
    {
        DEFAULT_UNIX_SOCKET.to_string()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RequestCommand {
    Ping,
    Fetch,
    GetProfile,
    Status,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Request {
    pub v: u32,
    pub cmd: RequestCommand,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
}

impl Request {
    pub fn ping() -> Self {
        Self {
            v: PROTOCOL_VERSION,
            cmd: RequestCommand::Ping,
            url: None,
        }
    }

    pub fn fetch(url: impl Into<String>) -> Self {
        Self {
            v: PROTOCOL_VERSION,
            cmd: RequestCommand::Fetch,
            url: Some(url.into()),
        }
    }

    pub fn get_profile() -> Self {
        Self {
            v: PROTOCOL_VERSION,
            cmd: RequestCommand::GetProfile,
            url: None,
        }
    }

    pub fn status() -> Self {
        Self {
            v: PROTOCOL_VERSION,
            cmd: RequestCommand::Status,
            url: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ResponseStatus {
    Ok,
    Err,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Response {
    pub v: u32,
    pub status: ResponseStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub msg: Option<String>,
}

impl Response {
    pub fn ok(data: Value) -> Self {
        Self {
            v: PROTOCOL_VERSION,
            status: ResponseStatus::Ok,
            data: Some(data),
            msg: None,
        }
    }

    pub fn err(message: impl Into<String>) -> Self {
        Self {
            v: PROTOCOL_VERSION,
            status: ResponseStatus::Err,
            data: None,
            msg: Some(message.into()),
        }
    }

    pub fn to_line(&self) -> Result<String, ProtocolError> {
        serde_json::to_string(self).map_err(|err| ProtocolError::Serialize(err.to_string()))
    }
}

pub type ResponseData = Value;

#[derive(Debug, Error)]
pub enum ProtocolError {
    #[error("unsupported protocol version: {0}")]
    UnsupportedVersion(u32),

    #[error("invalid request JSON: {0}")]
    InvalidRequest(String),

    #[error("failed to serialize response: {0}")]
    Serialize(String),

    #[error("missing required field: {0}")]
    MissingField(&'static str),

    #[error("IPC transport error: {0}")]
    Transport(String),
}

pub fn parse_request_line(line: &str) -> Result<Request, ProtocolError> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return Err(ProtocolError::InvalidRequest("empty line".into()));
    }

    let request: Request = serde_json::from_str(trimmed)
        .map_err(|err| ProtocolError::InvalidRequest(err.to_string()))?;

    if request.v != PROTOCOL_VERSION {
        return Err(ProtocolError::UnsupportedVersion(request.v));
    }

    if matches!(request.cmd, RequestCommand::Fetch)
        && request.url.as_deref().unwrap_or("").is_empty()
    {
        return Err(ProtocolError::MissingField("url"));
    }

    Ok(request)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_ping_request() {
        let line = serde_json::to_string(&Request::ping()).expect("serialize");
        let request = parse_request_line(&line).expect("parse");
        assert_eq!(request.cmd, RequestCommand::Ping);
    }

    #[test]
    fn fetch_requires_url() {
        let request = Request {
            v: PROTOCOL_VERSION,
            cmd: RequestCommand::Fetch,
            url: None,
        };
        let line = serde_json::to_string(&request).expect("serialize");
        let err = parse_request_line(&line).expect_err("missing url");
        assert!(matches!(err, ProtocolError::MissingField("url")));
    }

    #[test]
    fn response_line_format() {
        let response = Response::ok(serde_json::json!({"pong": true}));
        let line = response.to_line().expect("line");
        assert!(line.contains("\"status\":\"OK\""));
    }
}
