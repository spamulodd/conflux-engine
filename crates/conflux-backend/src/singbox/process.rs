use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::Duration;

use crate::error::BackendError;

const SINGBOX_PATH_ENV: &str = "SINGBOX_PATH";
const CONFLUX_SINGBOX_BIN_ENV: &str = "CONFLUX_SINGBOX_BIN";
const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(10);

/// Resolve the sing-box executable from environment variables or PATH.
pub fn resolve_singbox_binary() -> Result<PathBuf, BackendError> {
    if let Ok(path) = std::env::var(SINGBOX_PATH_ENV) {
        let candidate = PathBuf::from(path);
        if candidate.is_file() {
            return Ok(candidate);
        }
        return Err(BackendError::BinaryNotFound(format!(
            "{SINGBOX_PATH_ENV} points to missing file: {}",
            candidate.display()
        )));
    }

    if let Some(path) = resolve_singbox_next_to_daemon() {
        return Ok(path);
    }

    if let Ok(path) = std::env::var(CONFLUX_SINGBOX_BIN_ENV) {
        let candidate = PathBuf::from(path);
        if candidate.is_file() {
            return Ok(candidate);
        }
        return Err(BackendError::BinaryNotFound(format!(
            "{CONFLUX_SINGBOX_BIN_ENV} points to missing file: {}",
            candidate.display()
        )));
    }

    for name in ["sing-box", "sing-box.exe"] {
        if let Some(path) = find_on_path(name) {
            return Ok(path);
        }
    }

    Err(BackendError::BinaryNotFound(format!(
        "sing-box not found; set {SINGBOX_PATH_ENV}, bundle engines/sing-box.exe next to confluxd, or install sing-box on PATH"
    )))
}

/// Look for `engines/sing-box.exe` (or `sing-box.exe` in the same dir as confluxd).
fn resolve_singbox_next_to_daemon() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let dir = exe.parent()?;

    [
        dir.join("sing-box.exe"),
        dir.join("sing-box"),
        dir.join("engines").join("sing-box.exe"),
        dir.join("engines").join("sing-box"),
    ]
    .into_iter()
    .find(|candidate| candidate.is_file())
}

fn find_on_path(name: &str) -> Option<PathBuf> {
    let path_var = std::env::var_os("PATH")?;
    std::env::split_paths(&path_var)
        .map(|dir| dir.join(name))
        .find(|candidate| candidate.is_file())
}

/// Options for spawning sing-box.
#[derive(Debug, Clone)]
pub struct SingboxSpawnOptions {
    pub binary: PathBuf,
    pub config_path: PathBuf,
}

/// Managed sing-box subprocess handle.
pub struct SingboxProcess {
    child: Option<Child>,
    binary: PathBuf,
}

impl SingboxProcess {
    pub fn new(binary: PathBuf) -> Self {
        Self {
            child: None,
            binary,
        }
    }

    pub fn from_env() -> Result<Self, BackendError> {
        Ok(Self::new(resolve_singbox_binary()?))
    }

    pub fn binary(&self) -> &Path {
        &self.binary
    }

    pub fn is_running(&mut self) -> bool {
        match self.child.as_mut() {
            Some(child) => match child.try_wait() {
                Ok(Some(_)) => {
                    self.child = None;
                    false
                }
                Ok(None) => true,
                Err(_) => {
                    self.child = None;
                    false
                }
            },
            None => false,
        }
    }

    pub fn spawn(&mut self, options: &SingboxSpawnOptions) -> Result<(), BackendError> {
        if self.is_running() {
            return Err(BackendError::AlreadyRunning);
        }

        let child = Command::new(&options.binary)
            .arg("run")
            .arg("-c")
            .arg(&options.config_path)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|err| BackendError::Spawn(err.to_string()))?;

        self.binary = options.binary.clone();
        self.child = Some(child);
        Ok(())
    }

    pub fn stop(&mut self) -> Result<(), BackendError> {
        let Some(mut child) = self.child.take() else {
            return Ok(());
        };

        if let Err(err) = child.kill() {
            return Err(BackendError::Process(format!(
                "failed to stop sing-box: {err}"
            )));
        }

        let deadline = std::time::Instant::now() + SHUTDOWN_TIMEOUT;
        loop {
            match child.try_wait() {
                Ok(Some(_)) => return Ok(()),
                Ok(None) if std::time::Instant::now() >= deadline => {
                    let _ = child.kill();
                    child
                        .wait()
                        .map_err(|err| BackendError::Process(err.to_string()))?;
                    return Ok(());
                }
                Ok(None) => thread::sleep(Duration::from_millis(100)),
                Err(err) => return Err(BackendError::Process(err.to_string())),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_prefers_singbox_path_env() {
        let temp =
            std::env::temp_dir().join(format!("conflux-singbox-test-{}", std::process::id()));
        std::fs::write(&temp, b"").unwrap();

        unsafe {
            std::env::set_var(SINGBOX_PATH_ENV, &temp);
        }

        let resolved = resolve_singbox_binary().unwrap();
        assert_eq!(resolved, temp);

        unsafe {
            std::env::remove_var(SINGBOX_PATH_ENV);
        }
        let _ = std::fs::remove_file(temp);
    }
}
