//! Machine identity and persistent user-prompt settings routes, split out of [`super`] to keep
//! it under the repo's per-file line gate.

use super::AppState;
use crate::error::AppError;
use crate::routines;
use axum::{extract::State, http::StatusCode, Json};
use serde::{Deserialize, Serialize};

/// Response body for `GET /machine`.
#[derive(Serialize, utoipa::ToSchema)]
pub struct MachineResponse {
    /// Resolved name of this machine (from `MOADIM_MACHINE`, `~/.config/moadim/machine.local.toml`, or hostname).
    pub name: String,
}

/// `GET /machine` — the current machine's resolved identity.
///
/// Returns the name this daemon uses to match `machines[]` targeting lists on routines. Useful for
/// clients (e.g. the UI) that want to default their views to local entries only.
#[utoipa::path(get, path = "/machine",
    responses((status = 200, body = MachineResponse)))]
pub async fn get_current_machine() -> Json<MachineResponse> {
    Json(MachineResponse {
        name: crate::machine::current_machine(),
    })
}

/// Request body for `PUT /machine`.
#[derive(Deserialize, utoipa::ToSchema)]
pub struct SetMachineRequest {
    /// New machine name. Trimmed; must be non-empty.
    pub name: String,
}

/// `PUT /machine` — rename this machine's identity.
///
/// Writes the new name to `machine.local.toml` and returns it trimmed. Returns `400` if the name
/// is empty, `500` if the write fails. The `MOADIM_MACHINE` env var takes precedence at runtime;
/// setting the name here persists it for when the env var is absent.
///
/// As a side-effect, every routine whose `machines` list contained the old name is updated in
/// memory, on disk, and in the crontab so that the rename propagates atomically.
#[utoipa::path(put, path = "/machine",
    request_body = SetMachineRequest,
    responses(
        (status = 200, body = MachineResponse),
        (status = 400, description = "Empty name"),
        (status = 500, description = "Write failed"),
    ))]
pub async fn put_machine(
    State(state): State<AppState>,
    Json(body): Json<SetMachineRequest>,
) -> Result<Json<MachineResponse>, (StatusCode, String)> {
    let old_name = crate::machine::current_machine();
    let new_name = body.name.trim().to_string();
    match crate::machine::set_machine(&new_name) {
        Ok(()) => {
            routines::svc_rename_machine(&state.routines, &old_name, &new_name);
            Ok(Json(MachineResponse { name: new_name }))
        }
        Err(err) if err.kind() == std::io::ErrorKind::InvalidInput => {
            Err((StatusCode::BAD_REQUEST, err.to_string()))
        }
        Err(err) => Err((StatusCode::INTERNAL_SERVER_ERROR, err.to_string())),
    }
}

/// `GET /config/user-prompt` — the persistent system prompt appended to every routine's agent
/// instructions file (see [`crate::paths::user_prompt_path`]), as plain text. Empty (not an
/// error) when nothing has been saved yet.
#[utoipa::path(get, path = "/config/user-prompt",
    responses((status = 200, description = "User prompt contents as plain text")))]
pub async fn get_user_prompt() -> Result<String, AppError> {
    match std::fs::read_to_string(crate::paths::user_prompt_path()) {
        Ok(text) => Ok(text),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
        Err(_) => Err(AppError::Internal),
    }
}

/// Request body for `PUT /config/user-prompt`.
#[derive(Deserialize, utoipa::ToSchema)]
pub struct SetUserPromptRequest {
    /// New persistent prompt contents. An empty string clears it.
    pub content: String,
}

/// `PUT /config/user-prompt` — replace the persistent system prompt.
///
/// Creates the config directory if absent. Every routine's next run picks up the change (the
/// launch command re-reads this file each time — see `command::system_prompt_stmts`); already
/// running agents are unaffected.
#[utoipa::path(put, path = "/config/user-prompt",
    request_body = SetUserPromptRequest,
    responses((status = 204, description = "Saved"), (status = 500, description = "Write failed")))]
pub async fn put_user_prompt(
    Json(body): Json<SetUserPromptRequest>,
) -> Result<StatusCode, AppError> {
    let path = crate::paths::user_prompt_path();
    let parent = path.parent().ok_or(AppError::Internal)?;
    crate::utils::fs_perms::create_private_dir_all(parent).map_err(|_| AppError::Internal)?;
    std::fs::write(&path, &body.content).map_err(|_| AppError::Internal)?;
    Ok(StatusCode::NO_CONTENT)
}

/// Response body for `GET /config/max-concurrent-runs` and `PUT /config/max-concurrent-runs`.
#[derive(Serialize, utoipa::ToSchema)]
pub struct MaxConcurrentRunsResponse {
    /// Effective cap: the `MOADIM_MAX_CONCURRENT_RUNS` env var if set, else the persisted
    /// UI/REST override, else the built-in default. `0` means unbounded.
    pub value: usize,
    /// The persisted UI/REST override on its own, independent of the env var — `None` if never
    /// set. The UI should populate its input from this field (not `value`), so an env-var
    /// override in effect doesn't get echoed back as though it were a saved setting.
    pub override_value: Option<usize>,
}

/// `GET /config/max-concurrent-runs` — the global routine concurrency cap (issue #1155).
#[utoipa::path(get, path = "/config/max-concurrent-runs",
    responses((status = 200, body = MaxConcurrentRunsResponse)))]
pub async fn get_max_concurrent_runs() -> Json<MaxConcurrentRunsResponse> {
    Json(MaxConcurrentRunsResponse {
        value: routines::max_concurrent_runs(),
        override_value: crate::machine::max_concurrent_runs_override(),
    })
}

/// Request body for `PUT /config/max-concurrent-runs`.
#[derive(Deserialize, utoipa::ToSchema)]
pub struct SetMaxConcurrentRunsRequest {
    /// New override value, or `null`/omitted to clear it and fall back to the env var/default.
    pub value: Option<usize>,
}

/// `PUT /config/max-concurrent-runs` — set or clear the persisted concurrency-cap override.
///
/// Writes to `machine.local.toml`. The `MOADIM_MAX_CONCURRENT_RUNS` env var still takes
/// precedence at runtime when set; this only takes effect while the env var is unset. Takes
/// effect on the next trigger check (read live from disk, like the env var today) — no restart
/// required. Returns `500` if the write fails.
#[utoipa::path(put, path = "/config/max-concurrent-runs",
    request_body = SetMaxConcurrentRunsRequest,
    responses(
        (status = 200, body = MaxConcurrentRunsResponse),
        (status = 500, description = "Write failed"),
    ))]
pub async fn put_max_concurrent_runs(
    Json(body): Json<SetMaxConcurrentRunsRequest>,
) -> Result<Json<MaxConcurrentRunsResponse>, AppError> {
    crate::machine::set_max_concurrent_runs_override(body.value).map_err(|_| AppError::Internal)?;
    Ok(Json(MaxConcurrentRunsResponse {
        value: routines::max_concurrent_runs(),
        override_value: body.value,
    }))
}

/// `GET /machines` — distinct machine names this daemon knows about.
///
/// There is no central machine registry, so the "known" set is the union of every `machines`
/// targeting list declared by a routine, plus this machine's own resolved identity
/// ([`crate::machine::current_machine`]) so the local machine is always pickable even before
/// anything targets it. Sorted and de-duplicated. Backs the UI machine picker; mirrors the
/// `moadim machine list` CLI but reads the live in-memory store instead of disk.
#[utoipa::path(get, path = "/machines",
    responses((status = 200, body = Vec<String>, description = "Known machine names, sorted")))]
pub async fn list_machines(State(state): State<AppState>) -> Json<Vec<String>> {
    use crate::utils::lock::LockRecover;
    let mut names = std::collections::BTreeSet::new();
    names.insert(crate::machine::current_machine());
    for routine in state.routines.lock_recover().values() {
        names.extend(routine.machines.iter().cloned());
    }
    Json(names.into_iter().collect())
}
