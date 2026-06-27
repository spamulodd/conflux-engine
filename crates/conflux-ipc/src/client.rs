use conflux_core::ConfluxSubscription;
use serde_json::Value;

use crate::protocol::{
    default_endpoint, ProtocolError, Request, Response, ResponseStatus, PROTOCOL_VERSION,
};
use crate::server::exchange;

pub struct IpcClient {
    endpoint: String,
}

impl IpcClient {
    pub fn new(endpoint: impl Into<String>) -> Self {
        Self {
            endpoint: endpoint.into(),
        }
    }

    pub fn default_client() -> Self {
        Self::new(default_endpoint())
    }

    pub fn endpoint(&self) -> &str {
        &self.endpoint
    }

    pub async fn ping(&self) -> Result<Response, ProtocolError> {
        self.send(&Request::ping()).await
    }

    pub async fn fetch(&self, url: &str) -> Result<ConfluxSubscription, ProtocolError> {
        let response = self.send(&Request::fetch(url)).await?;
        response_into_profile(response)
    }

    pub async fn get_profile(&self) -> Result<ConfluxSubscription, ProtocolError> {
        let response = self.send(&Request::get_profile()).await?;
        response_into_profile(response)
    }

    pub async fn status(&self) -> Result<Value, ProtocolError> {
        let response = self.send(&Request::status()).await?;
        match response.status {
            ResponseStatus::Ok => response
                .data
                .ok_or_else(|| ProtocolError::Transport("status response missing data".into())),
            ResponseStatus::Err => Err(ProtocolError::Transport(
                response.msg.unwrap_or_else(|| "unknown IPC error".into()),
            )),
        }
    }

    pub async fn send(&self, request: &Request) -> Result<Response, ProtocolError> {
        if request.v != PROTOCOL_VERSION {
            return Err(ProtocolError::UnsupportedVersion(request.v));
        }

        #[cfg(windows)]
        {
            use tokio::net::windows::named_pipe::ClientOptions;
            let client = ClientOptions::new()
                .open(&self.endpoint)
                .map_err(|err| ProtocolError::Transport(err.to_string()))?;
            exchange(client, request).await
        }

        #[cfg(not(windows))]
        {
            use tokio::net::UnixStream;
            let stream = UnixStream::connect(&self.endpoint)
                .await
                .map_err(|err| ProtocolError::Transport(err.to_string()))?;
            exchange(stream, request).await
        }
    }
}

fn response_into_profile(response: Response) -> Result<ConfluxSubscription, ProtocolError> {
    match response.status {
        ResponseStatus::Ok => {
            let value = response
                .data
                .ok_or_else(|| ProtocolError::Transport("response missing data".into()))?;
            serde_json::from_value(value)
                .map_err(|err| ProtocolError::InvalidRequest(err.to_string()))
        }
        ResponseStatus::Err => Err(ProtocolError::Transport(
            response.msg.unwrap_or_else(|| "unknown IPC error".into()),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_client_uses_platform_endpoint() {
        let client = IpcClient::default_client();
        assert!(!client.endpoint().is_empty());
    }
}
