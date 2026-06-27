//! Shared daemon configuration and runtime for `confluxd` and CLI `daemon`.

mod config;

pub use config::{load_config, ConfigError, DaemonConfig, LoadedConfig, DEFAULT_CONFIG_PATH};

use std::sync::Arc;

use conflux_core::fetch_and_normalize;
use conflux_ipc::{default_endpoint, EngineState, IpcServer, DEFAULT_PIPE_NAME};
use thiserror::Error;
use tracing::{error, info};

#[derive(Debug, Error)]
pub enum DaemonError {
    #[error("configuration error: {0}")]
    Config(String),

    #[error("IPC server error: {0}")]
    Ipc(#[from] conflux_ipc::protocol::ProtocolError),

    #[error("subscription fetch error: {0}")]
    Fetch(#[from] conflux_core::ConfluxError),
}

/// Run the IPC daemon until the process is terminated.
pub async fn run_daemon(loaded: LoadedConfig) -> Result<(), DaemonError> {
    init_tracing();

    let endpoint = loaded
        .config
        .pipe_name
        .clone()
        .unwrap_or_else(default_endpoint);

    info!(
        endpoint = %endpoint,
        config_path = %loaded.loaded_from.display(),
        "starting conflux daemon"
    );

    let state = Arc::new(EngineState::new());

    if let Some(url) = loaded.config.subscription_url.as_deref() {
        match fetch_and_normalize(url).await {
            Ok(profile) => {
                *state.last_fetch_url.write().await = Some(url.to_string());
                *state.profile.write().await = Some(profile);
                info!("prefetched subscription from config");
            }
            Err(err) => {
                error!(error = %err, "initial subscription prefetch failed");
                *state.last_error.write().await = Some(err.to_string());
            }
        }
    }

    let server = IpcServer::with_state(endpoint, Arc::clone(&state));
    server.run().await?;
    Ok(())
}

fn init_tracing() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .try_init();
}

pub fn default_pipe_name() -> &'static str {
    DEFAULT_PIPE_NAME
}
