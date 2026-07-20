//! Shared `unlock_routines` logic: response shape and how to build it. Both the HTTP handler
//! (`http.rs`) and the MCP tool (`mcp.rs`) build on top of this.

use serde::Deserialize;

use crate::error::AppError;
use crate::global_lock::LockScope;
pub use crate::global_lock::LockStatus;
pub use crate::routines::RoutineStore;

/// Query parameters for `DELETE /routines/lock`.
#[derive(Deserialize, utoipa::IntoParams)]
pub struct UnlockQuery {
    /// Which sentinel(s) to remove: `"shared"`, `"local"`, or `"all"`.
    pub scope: String,
}

/// Parse a `scope` string into the [`LockScope`]s to remove, returning `400 BadRequest` on
/// unknown values.
fn parse_scopes(scope: &str) -> Result<Vec<LockScope>, AppError> {
    match scope {
        "shared" => Ok(vec![LockScope::Shared]),
        "local" => Ok(vec![LockScope::Local]),
        "all" => Ok(vec![LockScope::Shared, LockScope::Local]),
        other => Err(AppError::BadRequest(format!(
            "unknown scope {other:?}; use \"shared\", \"local\", or \"all\""
        ))),
    }
}

/// Remove lock sentinel(s) for `scope`, sync the crontab, and return the resulting lock status.
pub fn build(store: &RoutineStore, scope: &str) -> Result<LockStatus, AppError> {
    let scopes = parse_scopes(scope)?;
    for lock_scope in scopes {
        crate::global_lock::set_lock(lock_scope, false).map_err(|_| AppError::Internal)?;
    }
    if let Err(sync_err) = crate::sync::routines::sync_routines_to_crontab(store) {
        log::warn!("crontab sync after unlock failed: {sync_err}");
    }
    Ok(crate::global_lock::lock_status())
}

#[cfg(test)]
#[path = "logic_tests.rs"]
mod logic_tests;
