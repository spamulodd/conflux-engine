mod config;
mod process;

pub use config::{generate_config, redact_config, GenerateOptions};
pub use process::{resolve_singbox_binary, SingboxProcess, SingboxSpawnOptions};
