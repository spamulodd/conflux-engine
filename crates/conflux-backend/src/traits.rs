use conflux_protocol::ConfluxSubscription;

use crate::error::BackendError;
use crate::runtime::BackendHealth;

/// Pluggable proxy data-plane backend (sing-box, future mihomo, etc.).
pub trait Backend {
    /// Store a normalized profile and prepare backend configuration.
    fn apply_profile(
        &mut self,
        profile: &ConfluxSubscription,
        selected_node_id: Option<&str>,
    ) -> Result<(), BackendError>;

    /// Start the backend process using the last applied profile.
    fn start(&mut self) -> Result<(), BackendError>;

    /// Stop the backend process and release resources.
    fn stop(&mut self) -> Result<(), BackendError>;

    /// Current lifecycle state and optional detail message.
    fn health(&self) -> BackendHealth;
}
