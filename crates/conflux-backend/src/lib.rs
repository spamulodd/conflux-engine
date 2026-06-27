//! Proxy backend adapters (sing-box subprocess supervision).

mod error;
mod runtime;
mod singbox;
mod traits;

pub use error::BackendError;
pub use runtime::{BackendHealth, BackendState, SingboxBackend};
pub use singbox::{generate_config, redact_config, resolve_singbox_binary, GenerateOptions};
pub use traits::Backend;

pub use conflux_core;
pub use conflux_protocol;
