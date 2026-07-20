//! Shared `lock_routines` logic: response shape and how to build it. Both the HTTP handler
//! (`http.rs`) and the MCP tool (`mcp.rs`) build on top of this.

use serde::Deserialize;

use crate::error::AppError;
use crate::global_lock::LockScope;
pub use crate::global_lock::LockStatus;
pub use crate::routines::RoutineStore;

/// Request body for `POST /routines/lock`.
#[derive(Deserialize, utoipa::ToSchema)]
pub struct LockRequest {
    /// Which sentinel to create: `"shared"` (committed `.lock`) or `"local"` (gitignored `.local.lock`).
    pub scope: String,
}

/// Parse a `scope` string into a [`LockScope`], returning `400 BadRequest` on unknown values.
fn parse_scope(scope: &str) -> Result<LockScope, AppError> {
    match scope {
        "shared" => Ok(LockScope::Shared),
        "local" => Ok(LockScope::Local),
        other => Err(AppError::BadRequest(format!(
            "unknown scope {other:?}; use \"shared\" or \"local\""
        ))),
    }
}

/// Create a lock sentinel for `scope`, sync the crontab, and return the resulting lock status.
pub fn build(store: &RoutineStore, scope: &str) -> Result<LockStatus, AppError> {
    let lock_scope = parse_scope(scope)?;
    crate::global_lock::set_lock(lock_scope, true).map_err(|_| AppError::Internal)?;
    if let Err(sync_err) = crate::sync::routines::sync_routines_to_crontab(store) {
        log::warn!("crontab sync after lock failed: {sync_err}");
    }
    Ok(crate::global_lock::lock_status())
}

#[cfg(test)]
#[path = "logic_tests.rs"]
mod logic_tests;
