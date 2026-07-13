//! Routine data model, request/response types, and the `/routines` API client.

use gloo_net::http::Request;
use serde::{Deserialize, Serialize};

/// Agents the daemon ships built-in configs for (see `src/routines/agents`). Keep in sync with
/// `DEFAULT_AGENT_CONFIGS`.
pub const AVAILABLE_AGENTS: &[&str] = &["claude", "codex"];

// ─── Types (mirror server API exactly) ────────────────────────────────────────

/// A git repository listed in a routine's prompt as context.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Repository {
    pub repository: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
}

/// A routine as returned by `GET /routines` (the flattened `RoutineResponse`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Routine {
    pub id: String,
    pub schedule: String,
    pub title: String,
    pub agent: String,
    /// Model ID the agent runs with (e.g. `"claude-sonnet-4-6"`); `None` uses the agent's own
    /// default.
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub prompt: String,
    /// Short (≤5 line) statement of the routine's goal; `None` when unset.
    #[serde(default)]
    pub goal: Option<String>,
    #[serde(default)]
    pub repositories: Vec<Repository>,
    /// Machines this routine runs on. An empty list runs nowhere (dormant until assigned).
    #[serde(default)]
    pub machines: Vec<String>,
    pub enabled: bool,
    #[serde(default)]
    pub source: String,
    #[serde(default)]
    pub created_at: u64,
    #[serde(default)]
    pub updated_at: u64,
    #[serde(default)]
    pub last_manual_trigger_at: Option<u64>,
    #[serde(default)]
    pub last_scheduled_trigger_at: Option<u64>,
    /// Unix timestamp until which scheduled fires are skipped, or `None`. Mutually exclusive with
    /// `skip_runs`.
    #[serde(default)]
    pub snoozed_until: Option<u64>,
    /// Count of upcoming scheduled fires still to skip, or `None`. Mutually exclusive with
    /// `snoozed_until`.
    #[serde(default)]
    pub skip_runs: Option<u32>,
    /// Whether firing is paused for power saving, independent of `enabled`. System/policy-owned;
    /// not settable via create/update.
    #[serde(default)]
    pub power_saving: bool,
    /// Workbench retention (seconds) for finished runs; `None` falls back to the server default.
    #[serde(default)]
    pub ttl_secs: Option<u64>,
    /// Free-form labels for the routine.
    #[serde(default)]
    pub tags: Vec<String>,
    // Derived (absent on the bare Routine returned by /trigger — default to safe values).
    #[serde(default)]
    pub agent_registered: bool,
    #[serde(default)]
    pub file_path: String,
    #[serde(default)]
    pub schedule_description: Option<String>,
    /// Number of open flags raised against this routine (see [`Flag`]).
    #[serde(default)]
    pub flag_count: usize,
}

/// Whether a flag file is committed to version control or kept machine-local.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FlagScope {
    General,
    Local,
}

/// A flag raised against a routine (mirrors the server `Flag`).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Flag {
    pub filename: String,
    #[serde(rename = "type")]
    pub flag_type: String,
    pub description: String,
    pub scope: FlagScope,
    #[serde(default)]
    pub created_at: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct CreateRoutineRequest {
    pub schedule: String,
    pub title: String,
    pub agent: String,
    /// Model ID to run the agent with; `None` uses the agent's own default.
    pub model: Option<String>,
    pub prompt: String,
    /// Short (≤5 line) goal statement; `None` when unset.
    pub goal: Option<String>,
    pub repositories: Vec<Repository>,
    /// Machines to run this routine on (empty = runs nowhere until assigned).
    pub machines: Vec<String>,
    pub enabled: bool,
    /// Workbench retention (seconds); `None` lets the server apply its default.
    pub ttl_secs: Option<u64>,
    /// Free-form labels for the routine.
    pub tags: Vec<String>,
}

/// Result of `POST /routines/cleanup` (mirrors the server `CleanupResponse`).
#[derive(Debug, Clone, Deserialize)]
pub struct CleanupResponse {
    pub removed: usize,
    /// Disk space reclaimed by the sweep, in bytes. `#[serde(default)]` so a response from an older
    /// server that predates this field still deserializes (freed reads as 0).
    #[serde(default)]
    pub freed_bytes: u64,
}

/// Outcome of a single past run (mirrors the server `RunStatus`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunStatus {
    Running,
    Success,
    Failed,
    Unknown,
}

/// One past (or in-progress) run of a routine (mirrors the server `RunSummary`).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct RunSummary {
    pub workbench: String,
    pub started_at: u64,
    pub finished_at: Option<u64>,
    pub status: RunStatus,
    pub exit_code: Option<i32>,
    pub retention_expires_at: Option<u64>,
}

/// One past (or in-progress) run across every routine (mirrors the server `FleetRunSummary`).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct FleetRunSummary {
    pub routine_id: String,
    pub routine_title: String,
    pub workbench: String,
    pub started_at: u64,
    pub finished_at: Option<u64>,
    pub status: RunStatus,
    pub exit_code: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct UpdateRoutineRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub schedule: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub goal: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repositories: Option<Vec<Repository>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub machines: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ttl_secs: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
}

// ─── API layer ────────────────────────────────────────────────────────────────

pub(crate) async fn api_list() -> Result<Vec<Routine>, String> {
    Request::get("/api/v1/routines")
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json::<Vec<Routine>>()
        .await
        .map_err(|e| e.to_string())
}

pub(crate) async fn api_agents() -> Result<Vec<String>, String> {
    Request::get("/api/v1/agents")
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json::<Vec<String>>()
        .await
        .map_err(|e| e.to_string())
}

pub(crate) async fn api_create(req: &CreateRoutineRequest) -> Result<Routine, String> {
    let resp = Request::post("/api/v1/routines")
        .json(req)
        .map_err(|e| e.to_string())?
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !resp.ok() {
        return Err(format!("HTTP {}", resp.status()));
    }
    resp.json::<Routine>().await.map_err(|e| e.to_string())
}

pub(crate) async fn api_update(id: &str, req: &UpdateRoutineRequest) -> Result<Routine, String> {
    let resp = Request::patch(&format!("/api/v1/routines/{id}"))
        .json(req)
        .map_err(|e| e.to_string())?
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !resp.ok() {
        return Err(format!("HTTP {}", resp.status()));
    }
    resp.json::<Routine>().await.map_err(|e| e.to_string())
}

pub(crate) async fn api_delete(id: &str) -> Result<(), String> {
    let resp = Request::delete(&format!("/api/v1/routines/{id}"))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if resp.ok() {
        Ok(())
    } else {
        Err(format!("HTTP {}", resp.status()))
    }
}

pub(crate) async fn api_trigger(id: &str) -> Result<Routine, String> {
    let resp = Request::post(&format!("/api/v1/routines/{id}/trigger"))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !resp.ok() {
        return Err(format!("HTTP {}", resp.status()));
    }
    resp.json::<Routine>().await.map_err(|e| e.to_string())
}

/// Lock state as returned by `GET /api/v1/routines/lock`.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq, Default)]
pub struct LockStatus {
    pub shared: bool,
    pub local: bool,
    pub locked: bool,
}

pub(crate) async fn api_lock_status() -> Result<LockStatus, String> {
    Request::get("/api/v1/routines/lock")
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json::<LockStatus>()
        .await
        .map_err(|e| e.to_string())
}

pub(crate) async fn api_unlock(scope: &str) -> Result<LockStatus, String> {
    let resp = Request::delete(&format!("/api/v1/routines/lock?scope={scope}"))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !resp.ok() {
        return Err(format!("HTTP {}", resp.status()));
    }
    resp.json::<LockStatus>().await.map_err(|e| e.to_string())
}

pub(crate) async fn api_cleanup() -> Result<(usize, u64), String> {
    let resp = Request::post("/api/v1/routines/cleanup")
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !resp.ok() {
        return Err(format!("HTTP {}", resp.status()));
    }
    resp.json::<CleanupResponse>()
        .await
        .map(|r| (r.removed, r.freed_bytes))
        .map_err(|e| e.to_string())
}

/// Render a byte count as a short human-readable size (`B`/`KB`/`MB`/`GB`/`TB`, 1024-based): values
/// under 1 KiB show as a bare integer, larger ones with one decimal (`12.4 MB`). Mirrors the CLI's
/// `humanize_bytes` so the UI cleanup toast and `moadim cleanup` report the freed size identically.
pub(crate) fn humanize_bytes(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    if bytes < 1024 {
        return format!("{bytes} B");
    }
    let mut size = bytes as f64;
    let mut unit = 0;
    while size >= 1024.0 && unit < UNITS.len() - 1 {
        size /= 1024.0;
        unit += 1;
    }
    format!("{size:.1} {}", UNITS[unit])
}

pub(crate) async fn api_logs(id: &str) -> Result<String, String> {
    let resp = Request::get(&format!("/api/v1/routines/{id}/logs"))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !resp.ok() {
        return Err(format!("HTTP {}", resp.status()));
    }
    resp.text().await.map_err(|e| e.to_string())
}

pub(crate) async fn api_runs(id: &str) -> Result<Vec<RunSummary>, String> {
    let resp = Request::get(&format!("/api/v1/routines/{id}/runs"))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !resp.ok() {
        return Err(format!("HTTP {}", resp.status()));
    }
    resp.json::<Vec<RunSummary>>()
        .await
        .map_err(|e| e.to_string())
}

pub(crate) async fn api_run_log(id: &str, workbench: &str) -> Result<String, String> {
    let resp = Request::get(&format!("/api/v1/routines/{id}/runs/{workbench}/log"))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !resp.ok() {
        return Err(format!("HTTP {}", resp.status()));
    }
    resp.text().await.map_err(|e| e.to_string())
}

pub(crate) async fn api_all_runs(limit: usize) -> Result<Vec<FleetRunSummary>, String> {
    let resp = Request::get(&format!("/api/v1/routines/runs?limit={limit}"))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !resp.ok() {
        return Err(format!("HTTP {}", resp.status()));
    }
    resp.json::<Vec<FleetRunSummary>>()
        .await
        .map_err(|e| e.to_string())
}

pub(crate) async fn api_flags(id: &str) -> Result<Vec<Flag>, String> {
    let resp = Request::get(&format!("/api/v1/routines/{id}/flags"))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !resp.ok() {
        return Err(format!("HTTP {}", resp.status()));
    }
    resp.json::<Vec<Flag>>().await.map_err(|e| e.to_string())
}

pub(crate) async fn api_resolve_flag(id: &str, filename: &str) -> Result<(), String> {
    let resp = Request::delete(&format!("/api/v1/routines/{id}/flags/{filename}"))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if resp.ok() {
        Ok(())
    } else {
        Err(format!("HTTP {}", resp.status()))
    }
}

#[cfg(test)]
#[path = "model_tests.rs"]
mod model_tests;
