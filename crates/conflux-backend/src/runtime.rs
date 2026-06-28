use std::fs;
use std::path::{Path, PathBuf};

use conflux_protocol::ConfluxSubscription;
use serde_json::Value;

use crate::error::BackendError;
use crate::singbox::{generate_config, GenerateOptions, SingboxProcess, SingboxSpawnOptions};
use crate::traits::Backend;

/// Backend lifecycle states.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackendState {
    Idle,
    Starting,
    Running,
    Stopping,
    Error,
}

impl BackendState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Starting => "starting",
            Self::Running => "running",
            Self::Stopping => "stopping",
            Self::Error => "error",
        }
    }
}

/// Health snapshot returned by [`Backend::health`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackendHealth {
    pub state: BackendState,
    pub message: Option<String>,
}

impl BackendHealth {
    pub fn idle() -> Self {
        Self {
            state: BackendState::Idle,
            message: None,
        }
    }

    pub fn running() -> Self {
        Self {
            state: BackendState::Running,
            message: None,
        }
    }

    pub fn error(message: impl Into<String>) -> Self {
        Self {
            state: BackendState::Error,
            message: Some(message.into()),
        }
    }
}

/// sing-box backend runtime with an explicit state machine.
pub struct SingboxBackend {
    state: BackendState,
    profile: Option<ConfluxSubscription>,
    selected_node_id: Option<String>,
    config: Option<Value>,
    config_path: Option<PathBuf>,
    process: SingboxProcess,
    last_error: Option<String>,
    generate_options: GenerateOptions,
}

impl SingboxBackend {
    pub fn new() -> Result<Self, BackendError> {
        Ok(Self {
            state: BackendState::Idle,
            profile: None,
            selected_node_id: None,
            config: None,
            config_path: None,
            process: SingboxProcess::from_env()?,
            last_error: None,
            generate_options: GenerateOptions::for_windows(),
        })
    }

    pub fn with_binary(binary: PathBuf) -> Self {
        Self {
            state: BackendState::Idle,
            profile: None,
            selected_node_id: None,
            config: None,
            config_path: None,
            process: SingboxProcess::new(binary),
            last_error: None,
            generate_options: GenerateOptions::for_windows(),
        }
    }

    pub fn state(&self) -> BackendState {
        self.state
    }

    pub fn selected_node_id(&self) -> Option<&str> {
        self.selected_node_id.as_deref()
    }

    pub fn config(&self) -> Option<&Value> {
        self.config.as_ref()
    }

    fn transition(&mut self, next: BackendState) -> Result<(), BackendError> {
        if !is_valid_transition(self.state, next) {
            return Err(BackendError::InvalidTransition {
                from: self.state.as_str().to_string(),
                to: next.as_str().to_string(),
            });
        }
        self.state = next;
        Ok(())
    }

    fn write_config(&self, config: &Value) -> Result<PathBuf, BackendError> {
        let dir = std::env::temp_dir().join("conflux-backend");
        ensure_secret_dir(&dir)?;

        let path = dir.join(format!("singbox-{}.json", std::process::id()));
        let body = serde_json::to_vec_pretty(config)
            .map_err(|err| BackendError::Config(err.to_string()))?;
        write_secret_file(&path, &body)?;
        Ok(path)
    }

    fn cleanup_config(&mut self) -> Result<(), BackendError> {
        if let Some(path) = self.config_path.take() {
            let _ = fs::remove_file(path);
        }
        Ok(())
    }

    fn refresh_running_state(&mut self) {
        if self.state == BackendState::Running && !self.process.is_running() {
            self.state = BackendState::Error;
            self.last_error = Some("sing-box process exited unexpectedly".to_string());
        }
    }

    /// Refresh lifecycle state before IPC status or connect handlers read it.
    pub fn refresh_running_state_for_ipc(&mut self) {
        self.refresh_running_state();
    }
}

impl Default for SingboxBackend {
    fn default() -> Self {
        Self::with_binary(PathBuf::from("sing-box"))
    }
}

impl Backend for SingboxBackend {
    fn apply_profile(
        &mut self,
        profile: &ConfluxSubscription,
        selected_node_id: Option<&str>,
    ) -> Result<(), BackendError> {
        self.refresh_running_state();
        if matches!(self.state, BackendState::Running | BackendState::Starting) {
            return Err(BackendError::AlreadyRunning);
        }

        let mut options = self.generate_options.clone();
        options.selected_node_id = selected_node_id.map(str::to_string);

        let config = generate_config(profile, &options)?;
        self.profile = Some(profile.clone());
        self.selected_node_id = options.selected_node_id;
        self.config = Some(config);
        self.last_error = None;

        if self.state == BackendState::Error {
            self.state = BackendState::Idle;
        }

        Ok(())
    }

    fn start(&mut self) -> Result<(), BackendError> {
        self.refresh_running_state();

        match self.state {
            BackendState::Running => return Err(BackendError::AlreadyRunning),
            BackendState::Starting | BackendState::Stopping => {
                return Err(BackendError::InvalidTransition {
                    from: self.state.as_str().to_string(),
                    to: BackendState::Running.as_str().to_string(),
                });
            }
            BackendState::Idle | BackendState::Error => {}
        }

        let config = self.config.clone().ok_or(BackendError::NoProfile)?;

        self.transition(BackendState::Starting)?;

        let config_path = match self.write_config(&config) {
            Ok(path) => path,
            Err(err) => {
                self.state = BackendState::Error;
                self.last_error = Some(err.to_string());
                return Err(err);
            }
        };

        let binary = self.process.binary().to_path_buf();
        let spawn_result = self.process.spawn(&SingboxSpawnOptions {
            binary,
            config_path: config_path.clone(),
        });

        if let Err(err) = spawn_result {
            self.state = BackendState::Error;
            self.last_error = Some(err.to_string());
            let _ = fs::remove_file(&config_path);
            return Err(err);
        }

        self.config_path = Some(config_path);
        self.transition(BackendState::Running)?;

        std::thread::sleep(std::time::Duration::from_millis(400));
        self.refresh_running_state();
        if self.state == BackendState::Error {
            let message = self
                .last_error
                .clone()
                .unwrap_or_else(|| "sing-box exited immediately after start".to_string());
            return Err(BackendError::Process(message));
        }

        self.last_error = None;
        Ok(())
    }

    fn stop(&mut self) -> Result<(), BackendError> {
        self.refresh_running_state();

        match self.state {
            BackendState::Idle => return Ok(()),
            BackendState::Error => {
                let _ = self.process.stop();
                self.cleanup_config()?;
                self.state = BackendState::Idle;
                self.last_error = None;
                return Ok(());
            }
            BackendState::Stopping => {
                return Err(BackendError::InvalidTransition {
                    from: self.state.as_str().to_string(),
                    to: BackendState::Idle.as_str().to_string(),
                });
            }
            BackendState::Starting | BackendState::Running => {}
        }

        self.transition(BackendState::Stopping)?;

        if let Err(err) = self.process.stop() {
            self.state = BackendState::Error;
            self.last_error = Some(err.to_string());
            return Err(err);
        }

        self.cleanup_config()?;
        self.transition(BackendState::Idle)?;
        self.last_error = None;
        Ok(())
    }

    fn health(&self) -> BackendHealth {
        BackendHealth {
            state: self.state,
            message: self.last_error.clone(),
        }
    }
}

fn ensure_secret_dir(dir: &Path) -> Result<(), BackendError> {
    fs::create_dir_all(dir).map_err(BackendError::Io)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(dir, fs::Permissions::from_mode(0o700)).map_err(BackendError::Io)?;
    }
    Ok(())
}

fn write_secret_file(path: &Path, body: &[u8]) -> Result<(), BackendError> {
    fs::write(path, body).map_err(BackendError::Io)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o600)).map_err(BackendError::Io)?;
    }
    Ok(())
}

fn is_valid_transition(from: BackendState, to: BackendState) -> bool {
    matches!(
        (from, to),
        (BackendState::Idle, BackendState::Starting)
            | (BackendState::Starting, BackendState::Running)
            | (BackendState::Starting, BackendState::Error)
            | (BackendState::Running, BackendState::Stopping)
            | (BackendState::Running, BackendState::Error)
            | (BackendState::Stopping, BackendState::Idle)
            | (BackendState::Stopping, BackendState::Error)
            | (BackendState::Error, BackendState::Idle)
            | (BackendState::Error, BackendState::Starting)
            | (BackendState::Idle, BackendState::Idle)
            | (BackendState::Running, BackendState::Running)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[test]
    fn secret_files_are_not_world_readable() {
        use std::os::unix::fs::PermissionsExt;

        let dir =
            std::env::temp_dir().join(format!("conflux-backend-perm-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);

        ensure_secret_dir(&dir).expect("create secret dir");
        let path = dir.join("singbox-test.json");
        write_secret_file(&path, br#"{"password":"secret"}"#).expect("write secret file");

        let mode = fs::metadata(&path).expect("metadata").permissions().mode() & 0o777;
        assert_eq!(mode, 0o600, "config must be owner-read/write only");

        let dir_mode = fs::metadata(&dir)
            .expect("dir metadata")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(dir_mode, 0o700, "config dir must be owner-only");

        let _ = fs::remove_file(&path);
        let _ = fs::remove_dir(&dir);
    }

    #[test]
    fn state_transitions_are_restricted() {
        assert!(is_valid_transition(
            BackendState::Idle,
            BackendState::Starting
        ));
        assert!(is_valid_transition(
            BackendState::Starting,
            BackendState::Running
        ));
        assert!(is_valid_transition(
            BackendState::Running,
            BackendState::Stopping
        ));
        assert!(!is_valid_transition(
            BackendState::Idle,
            BackendState::Running
        ));
    }
}
