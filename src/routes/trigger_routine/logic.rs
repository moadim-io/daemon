//! Shared `trigger_routine` logic: response shape and how to build it. Both the HTTP handler
//! (`http.rs`) and the MCP tool (`mcp.rs`) build on top of this.

use crate::error::AppError;
pub(crate) use crate::routines::{Routine, RoutineStore};
use serde::Deserialize;
use utoipa::ToSchema;

/// An explicit operator confirmation to run despite host-level power saving.
#[derive(Debug, Deserialize, ToSchema)]
pub struct TriggerRoutineRequest {
    /// Bypass only host-level power saving for this manual run. It does not bypass a routine's
    /// explicit power-saving state, disabled state, or the global lock.
    #[serde(default)]
    pub override_system_power_saving: bool,
}

/// Manually trigger the routine with the given UUID, recording `last_manual_trigger_at` and
/// returning the updated record.
pub fn build(
    store: &RoutineStore,
    id: &str,
    override_system_power_saving: bool,
) -> Result<Routine, AppError> {
    crate::routines::svc_trigger_with_system_power_saving_override(
        store,
        id,
        override_system_power_saving,
    )
}

#[cfg(test)]
#[path = "logic_tests.rs"]
mod logic_tests;
