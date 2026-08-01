//! Shared failure notification hook configuration types.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Opt-in hooks fired once when a routine run finishes abnormally.
#[derive(
    Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema, utoipa::ToSchema,
)]
pub struct FailureNotificationConfig {
    /// Shell command run with `MOADIM_*` context env vars when a run fails.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on_failure_command: Option<String>,
    /// Webhook URL posted with a small JSON failure payload when a run fails.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on_failure_webhook: Option<String>,
}

impl FailureNotificationConfig {
    /// Whether no failure hook is configured.
    pub const fn is_empty(&self) -> bool {
        self.on_failure_command.is_none() && self.on_failure_webhook.is_none()
    }
}
