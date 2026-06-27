use thiserror::Error;

#[derive(Debug, Error)]
pub enum BackendError {
    #[error("backend is already running")]
    AlreadyRunning,

    #[error("backend is not running")]
    NotRunning,

    #[error("no profile applied")]
    NoProfile,

    #[error("no selectable node in profile")]
    NoNode,

    #[error("node not found: {0}")]
    NodeNotFound(String),

    #[error("unsupported protocol for sing-box: {0}")]
    UnsupportedProtocol(String),

    #[error("config generation failed: {0}")]
    Config(String),

    #[error("sing-box binary not found: {0}")]
    BinaryNotFound(String),

    #[error("failed to spawn sing-box: {0}")]
    Spawn(String),

    #[error("sing-box process error: {0}")]
    Process(String),

    #[error("invalid backend state transition from {from} to {to}")]
    InvalidTransition { from: String, to: String },

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}
