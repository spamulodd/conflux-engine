use std::path::PathBuf;

use serde::Deserialize;
use thiserror::Error;

pub const DEFAULT_CONFIG_PATH: &str = "conflux.toml";

#[derive(Debug, Clone, Deserialize, Default)]
pub struct DaemonConfig {
    #[serde(default)]
    pub subscription_url: Option<String>,
    #[serde(default)]
    pub pipe_name: Option<String>,
}

impl DaemonConfig {
    pub fn default_with_pipe() -> Self {
        Self {
            subscription_url: None,
            pipe_name: Some(conflux_ipc::DEFAULT_PIPE_NAME.to_string()),
        }
    }
}

#[derive(Debug)]
pub struct LoadedConfig {
    pub config: DaemonConfig,
    pub loaded_from: PathBuf,
}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("failed to read config {path}: {source}")]
    Read {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("failed to parse config {path}: {source}")]
    Parse {
        path: PathBuf,
        source: Box<toml::de::Error>,
    },
}

/// Load daemon configuration from `CONFLUX_CONFIG` or the default path.
pub fn load_config() -> Result<LoadedConfig, ConfigError> {
    let path = config_path();
    if path.exists() {
        let text = std::fs::read_to_string(&path).map_err(|source| ConfigError::Read {
            path: path.clone(),
            source,
        })?;
        let config: DaemonConfig = toml::from_str(&text).map_err(|source| ConfigError::Parse {
            path: path.clone(),
            source: Box::new(source),
        })?;
        Ok(LoadedConfig {
            config,
            loaded_from: path,
        })
    } else {
        Ok(LoadedConfig {
            config: DaemonConfig::default_with_pipe(),
            loaded_from: path,
        })
    }
}

fn config_path() -> PathBuf {
    if let Ok(path) = std::env::var("CONFLUX_CONFIG") {
        if !path.trim().is_empty() {
            return PathBuf::from(path);
        }
    }

    default_config_path()
}

fn default_config_path() -> PathBuf {
    if let Some(base) = platform_config_dir() {
        base.join("conflux.toml")
    } else {
        PathBuf::from(DEFAULT_CONFIG_PATH)
    }
}

fn platform_config_dir() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        std::env::var_os("APPDATA").map(PathBuf::from)
    }
    #[cfg(not(windows))]
    {
        std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config").join("conflux"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_has_pipe_name() {
        let config = DaemonConfig::default_with_pipe();
        assert_eq!(
            config.pipe_name.as_deref(),
            Some(conflux_ipc::DEFAULT_PIPE_NAME)
        );
    }
}
