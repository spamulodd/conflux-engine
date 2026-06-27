use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConfluxError {
    #[error("HTTP request failed: {0}")]
    Http(String),

    #[error("invalid subscription URL: {0}")]
    InvalidUrl(String),

    #[error("failed to parse subscription body: {0}")]
    Parse(String),

    #[error("failed to normalize node: {0}")]
    Normalize(String),

    #[error("unsupported protocol: {0}")]
    UnsupportedProtocol(String),

    #[error("invalid URI: {0}")]
    InvalidUri(String),
}
