//! Routine data model, agent registry, command builder, service functions, and HTTP handlers.
//!
//! A *routine* is a scheduled AI-agent task. Unlike a [`crate::cron_jobs::CronJob`] (which runs a
//! handler script), a routine launches an agent (claude code, codex, …) inside an interactive tmux
//! session rooted in a fresh workbench. moadim never clones the routine's `repositories`; it lists
//! them in the prompt as context and the agent clones any it needs.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use croner::Cron;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

use crate::cron_jobs::{normalize_schedule, validate_cron};
use crate::error::AppError;
use crate::paths::{agent_toml_path, routine_prompt_path, routine_toml_path, workbenches_dir};
use crate::routine_storage::{remove_routine_dir, write_routine};
use crate::utils::time::now_secs;

// ─── Data model ──────────────────────────────────────────────────────────────

/// A git repository made available to a routine's agent as prompt context (not cloned by moadim).
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, utoipa::ToSchema)]
pub struct Repository {
    /// Git remote URL.
    pub repository: String,
    /// Branch to use, or `None` for the remote default branch.
    #[serde(default)]
    pub branch: Option<String>,
}

/// A persisted routine: a scheduled AI-agent task.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, utoipa::ToSchema)]
pub struct Routine {
    /// Unique identifier (UUID v4).
    pub id: String,
    /// Cron expression defining when the routine runs.
    pub schedule: String,
    /// Human name; slugified to name the workbench and tmux session.
    pub title: String,
    /// Agent registry key (e.g. `"claude"`) resolved from `~/.config/moadim/agents/`.
    pub agent: String,
    /// The task prompt handed to the agent.
    pub prompt: String,
    /// Repositories listed in the prompt as context.
    #[serde(default)]
    pub repositories: Vec<Repository>,
    /// Whether the routine is active.
    pub enabled: bool,
    /// `"managed"` for routines owned by this server.
    pub source: String,
    /// Unix timestamp (seconds) when the routine was created.
    pub created_at: u64,
    /// Unix timestamp (seconds) when the routine was last updated.
    pub updated_at: u64,
    /// Unix timestamp (seconds) when the routine was last manually triggered, if ever.
    pub last_triggered_at: Option<u64>,
}

/// A [`Routine`] enriched with derived, non-persisted fields for API responses.
#[derive(Debug, Clone, Serialize, JsonSchema, utoipa::ToSchema)]
pub struct RoutineResponse {
    /// The underlying routine.
    #[serde(flatten)]
    pub routine: Routine,
    /// `true` if an agent config exists at `~/.config/moadim/agents/<agent>.toml`.
    pub agent_registered: bool,
    /// Absolute path to the routine's `routine.toml` file on disk.
    pub file_path: String,
    /// Human-readable description of the schedule, or `null` if it cannot be parsed.
    pub schedule_description: Option<String>,
}

impl RoutineResponse {
    /// Build a response from `routine`, deriving registration status and schedule description.
    pub fn from_routine(routine: Routine) -> Self {
        let agent_registered = agent_toml_path(&routine.agent).exists();
        let file_path = routine_toml_path(&routine.id)
            .to_string_lossy()
            .into_owned();
        let schedule_description = routine.schedule.parse::<Cron>().ok().map(|c| c.describe());
        Self {
            routine,
            agent_registered,
            file_path,
            schedule_description,
        }
    }
}

/// Thread-safe shared store of routines keyed by ID.
pub type RoutineStore = Arc<Mutex<HashMap<String, Routine>>>;

/// Create an empty [`RoutineStore`].
#[cfg(test)]
pub fn new_store() -> RoutineStore {
    Arc::new(Mutex::new(HashMap::new()))
}

// ─── Request bodies ──────────────────────────────────────────────────────────

/// Serde default for boolean fields that should default to `true`.
fn bool_true() -> bool {
    true
}

/// Request body for creating a new routine.
#[derive(Deserialize, JsonSchema, utoipa::ToSchema)]
pub struct CreateRoutineRequest {
    /// Cron expression for the new routine.
    pub schedule: String,
    /// Human name for the routine.
    pub title: String,
    /// Agent registry key to launch.
    pub agent: String,
    /// Task prompt.
    pub prompt: String,
    /// Repositories to list as context (defaults to empty).
    #[serde(default)]
    pub repositories: Vec<Repository>,
    /// Whether to create the routine enabled (defaults to `true`).
    #[serde(default = "bool_true")]
    pub enabled: bool,
}

/// Request body for partially updating an existing routine.
#[derive(Deserialize, JsonSchema, utoipa::ToSchema)]
pub struct UpdateRoutineRequest {
    /// New cron expression, or `None` to keep the existing value.
    pub schedule: Option<String>,
    /// New title, or `None` to keep the existing value.
    pub title: Option<String>,
    /// New agent key, or `None` to keep the existing value.
    pub agent: Option<String>,
    /// New prompt, or `None` to keep the existing value.
    pub prompt: Option<String>,
    /// New repositories list, or `None` to keep the existing value.
    pub repositories: Option<Vec<Repository>>,
    /// New enabled state, or `None` to keep the existing value.
    pub enabled: Option<bool>,
}

// ─── Agent registry ──────────────────────────────────────────────────────────

/// A resolved agent invocation read from `~/.config/moadim/agents/<name>.toml`.
#[derive(Debug, Clone, Deserialize)]
pub struct AgentCommand {
    /// Executable to run (e.g. `"claude"`).
    pub command: String,
    /// Arguments passed to the executable. Supports `{workbench}`, `{prompt_file}`, and `{prompt}`
    /// placeholders; `{prompt}` inlines the composed prompt as a single shell-quoted argument.
    #[serde(default)]
    pub args: Vec<String>,
    /// Optional shell command run in the workbench *before* the agent launches, inserted verbatim
    /// into the cron line. Runs with the shell vars `$WB` (absolute workbench path) and `$SESS`
    /// (tmux session name) in scope — e.g. to pre-seed per-directory editor trust state.
    #[serde(default)]
    pub setup: Option<String>,
}

/// Load the agent command for `name`, returning `None` if the config is missing or invalid.
pub fn load_agent_command(name: &str) -> Option<AgentCommand> {
    let text = std::fs::read_to_string(agent_toml_path(name)).ok()?;
    toml::from_str(&text).ok()
}

// ─── Prompt / command construction ───────────────────────────────────────────

/// Slugify `title` into a filesystem- and tmux-safe identifier.
///
/// Lowercases, replaces each run of non-alphanumeric characters with a single `-`, and trims
/// leading/trailing `-`. Returns `"routine"` if nothing usable remains.
pub(crate) fn slugify(title: &str) -> String {
    let mut out = String::new();
    let mut prev_dash = false;
    for c in title.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
            prev_dash = false;
        } else if !prev_dash {
            out.push('-');
            prev_dash = true;
        }
    }
    let trimmed = out.trim_matches('-').to_string();
    if trimmed.is_empty() {
        "routine".to_string()
    } else {
        trimmed
    }
}

/// Compose the `prompt.txt` body: a repositories-as-context preamble followed by the prompt.
pub(crate) fn compose_prompt(routine: &Routine) -> String {
    let mut s = String::from("# Workbench\n");
    s.push_str(
        "You are working in an empty directory. These repositories are relevant — clone any you need:\n",
    );
    for repo in &routine.repositories {
        match &repo.branch {
            Some(b) => s.push_str(&format!("- {} (branch {})\n", repo.repository, b)),
            None => s.push_str(&format!("- {}\n", repo.repository)),
        }
    }
    s.push_str("\n---\n");
    s.push_str(&routine.prompt);
    s.push('\n');
    s
}

/// Substitute `{workbench}`, `{prompt_file}`, and `{prompt}` placeholders in `s`.
///
/// `{prompt}` expands to a shell command substitution that reads `prompt.txt` from the agent's
/// cwd (the workbench), so the full prompt is passed as a single argument to the agent process.
fn substitute(s: &str, workbench: &str, prompt_file: &str) -> String {
    s.replace("{workbench}", workbench)
        .replace("{prompt_file}", prompt_file)
        .replace("{prompt}", r#""$(cat prompt.txt)""#)
}

/// Wrap `s` in single quotes for safe inclusion in a POSIX shell command.
fn shell_quote(s: &str) -> String {
    let mut out = String::from("'");
    for c in s.chars() {
        if c == '\'' {
            out.push_str("'\\''");
        } else {
            out.push(c);
        }
    }
    out.push('\'');
    out
}

/// Build the single-line shell command that creates a workbench and launches the agent in tmux.
///
/// The agent's cwd is the workbench (via `tmux -c`), so `{prompt_file}` resolves to `prompt.txt`,
/// `{workbench}` to `.`, and `{prompt}` to the prompt's contents passed as one argument. The prompt
/// reaches the agent as a process argument (not keystrokes), so there is no readiness race. The
/// command is `;`-joined (no newlines) so it fits one crontab line.
pub(crate) fn build_routine_command(routine: &Routine, agent: &AgentCommand) -> String {
    let slug = slugify(&routine.title);
    let prompt_path = routine_prompt_path(&routine.id)
        .to_string_lossy()
        .into_owned();

    let prompt_file_ref = "prompt.txt";
    let workbench_ref = ".";

    let mut invocation = vec![agent.command.clone()];
    for a in &agent.args {
        invocation.push(substitute(a, workbench_ref, prompt_file_ref));
    }
    let invocation = invocation.join(" ");

    let mut stmts = vec![
        r#"TS="$(date +%s)""#.to_string(),
        format!("SLUG={}", shell_quote(&slug)),
        r#"WB="$HOME/.moadim/workbenches/$SLUG-$TS""#.to_string(),
        r#"SESS="moadim-$SLUG-$TS""#.to_string(),
        r#"mkdir -p "$WB""#.to_string(),
        format!(r#"cp {} "$WB/prompt.txt""#, shell_quote(&prompt_path)),
    ];
    if let Some(setup) = &agent.setup {
        // Inserted verbatim so the agent author controls quoting; `$WB`/`$SESS` are in scope.
        stmts.push(setup.clone());
    }
    stmts.push(format!(
        r#"tmux new-session -d -s "$SESS" -c "$WB" {}"#,
        shell_quote(&invocation)
    ));
    stmts.push(r#"tmux pipe-pane -o -t "$SESS" "cat >> \"$WB\"/agent.log""#.to_string());
    stmts.join("; ")
}

// ─── Service layer (no HTTP types) ───────────────────────────────────────────

/// Return all routines sorted by creation time (oldest first).
pub fn svc_list(store: &RoutineStore) -> Vec<RoutineResponse> {
    let lock = store.lock().unwrap();
    let mut routines: Vec<Routine> = lock.values().cloned().collect();
    routines.sort_by_key(|r| r.created_at);
    drop(lock);
    routines
        .into_iter()
        .map(RoutineResponse::from_routine)
        .collect()
}

/// Look up a routine by `id`, returning `NotFound` if it does not exist.
pub fn svc_get(store: &RoutineStore, id: &str) -> Result<RoutineResponse, AppError> {
    let routine = store
        .lock()
        .unwrap()
        .get(id)
        .cloned()
        .ok_or(AppError::NotFound)?;
    Ok(RoutineResponse::from_routine(routine))
}

/// Validate `req`, assign a UUID, persist (routine.toml + prompt.txt), and sync the crontab.
pub fn svc_create(
    store: &RoutineStore,
    req: CreateRoutineRequest,
) -> Result<RoutineResponse, AppError> {
    validate_cron(&req.schedule)?;
    let now = now_secs();
    let routine = Routine {
        id: Uuid::new_v4().to_string(),
        schedule: normalize_schedule(&req.schedule),
        title: req.title,
        agent: req.agent,
        prompt: req.prompt,
        repositories: req.repositories,
        enabled: req.enabled,
        source: "managed".to_string(),
        created_at: now,
        updated_at: now,
        last_triggered_at: None,
    };
    write_routine(&routine).map_err(|_| AppError::Internal)?;
    store
        .lock()
        .unwrap()
        .insert(routine.id.clone(), routine.clone());
    if let Err(e) = crate::sync::routines::sync_routines_to_crontab(store) {
        log::warn!("crontab sync after routine create failed: {e}");
    }
    Ok(RoutineResponse::from_routine(routine))
}

/// Apply non-`None` fields from `req` to the routine identified by `id`.
pub fn svc_update(
    store: &RoutineStore,
    id: &str,
    req: UpdateRoutineRequest,
) -> Result<RoutineResponse, AppError> {
    if let Some(ref sched) = req.schedule {
        validate_cron(sched)?;
    }
    let mut lock = store.lock().unwrap();
    let routine = lock.get_mut(id).ok_or(AppError::NotFound)?;
    if let Some(s) = req.schedule {
        routine.schedule = normalize_schedule(&s);
    }
    if let Some(t) = req.title {
        routine.title = t;
    }
    if let Some(a) = req.agent {
        routine.agent = a;
    }
    if let Some(p) = req.prompt {
        routine.prompt = p;
    }
    if let Some(r) = req.repositories {
        routine.repositories = r;
    }
    if let Some(e) = req.enabled {
        routine.enabled = e;
    }
    routine.updated_at = now_secs();
    let routine = routine.clone();
    drop(lock);
    write_routine(&routine).map_err(|_| AppError::Internal)?;
    if let Err(e) = crate::sync::routines::sync_routines_to_crontab(store) {
        log::warn!("crontab sync after routine update failed: {e}");
    }
    Ok(RoutineResponse::from_routine(routine))
}

/// Remove the routine with `id` from the store and disk, then sync the crontab.
pub fn svc_delete(store: &RoutineStore, id: &str) -> Result<RoutineResponse, AppError> {
    let routine = store.lock().unwrap().remove(id).ok_or(AppError::NotFound)?;
    remove_routine_dir(id).map_err(|_| AppError::Internal)?;
    if let Err(e) = crate::sync::routines::sync_routines_to_crontab(store) {
        log::warn!("crontab sync after routine delete failed: {e}");
    }
    Ok(RoutineResponse::from_routine(routine))
}

/// Record a manual trigger for `id` and spawn the same command the crontab would run.
pub fn svc_trigger(store: &RoutineStore, id: &str) -> Result<Routine, AppError> {
    let mut lock = store.lock().unwrap();
    let routine = lock.get_mut(id).ok_or(AppError::NotFound)?;
    routine.last_triggered_at = Some(now_secs());
    let routine = routine.clone();
    drop(lock);
    write_routine(&routine).map_err(|_| AppError::Internal)?;
    match load_agent_command(&routine.agent) {
        Some(agent) => {
            let cmd = build_routine_command(&routine, &agent);
            if let Err(e) = std::process::Command::new("sh").arg("-c").arg(&cmd).spawn() {
                log::warn!("trigger: failed to spawn routine command: {e}");
            }
        }
        None => log::warn!(
            "trigger: agent config not found for routine {:?} (agent {:?})",
            routine.id,
            routine.agent
        ),
    }
    Ok(routine)
}

/// Return the contents of the newest workbench `agent.log` for routine `id`.
pub fn svc_logs(store: &RoutineStore, id: &str) -> Result<String, AppError> {
    let routine = store
        .lock()
        .unwrap()
        .get(id)
        .cloned()
        .ok_or(AppError::NotFound)?;
    let prefix = format!("{}-", slugify(&routine.title));
    let mut newest: Option<String> = None;
    if let Ok(entries) = std::fs::read_dir(workbenches_dir()) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.starts_with(&prefix) && newest.as_ref().is_none_or(|n| name > *n) {
                newest = Some(name);
            }
        }
    }
    let Some(dir) = newest else {
        return Ok(String::new());
    };
    let log_path = workbenches_dir().join(dir).join("agent.log");
    if !log_path.exists() {
        return Ok(String::new());
    }
    std::fs::read_to_string(&log_path).map_err(|_| AppError::Internal)
}

// ─── Axum HTTP handlers ──────────────────────────────────────────────────────

/// `POST /routines` — create a new routine.
#[utoipa::path(post, path = "/routines",
    request_body = CreateRoutineRequest,
    responses((status = 201, body = RoutineResponse), (status = 400, description = "Invalid cron expression")))]
pub async fn create(
    State(store): State<RoutineStore>,
    Json(body): Json<CreateRoutineRequest>,
) -> Result<(StatusCode, Json<RoutineResponse>), AppError> {
    Ok((StatusCode::CREATED, Json(svc_create(&store, body)?)))
}

/// `GET /routines` — list all routines sorted by creation time.
#[utoipa::path(get, path = "/routines",
    responses((status = 200, body = Vec<RoutineResponse>)))]
pub async fn list(State(store): State<RoutineStore>) -> Json<Vec<RoutineResponse>> {
    Json(svc_list(&store))
}

/// `GET /routines/{id}` — retrieve a single routine by UUID.
#[utoipa::path(get, path = "/routines/{id}",
    params(("id" = String, Path, description = "Routine UUID")),
    responses((status = 200, body = RoutineResponse), (status = 404, description = "Not found")))]
pub async fn get(
    State(store): State<RoutineStore>,
    Path(id): Path<String>,
) -> Result<Json<RoutineResponse>, AppError> {
    Ok(Json(svc_get(&store, &id)?))
}

/// `PATCH /routines/{id}` — partially update a routine.
#[utoipa::path(patch, path = "/routines/{id}",
    params(("id" = String, Path, description = "Routine UUID")),
    request_body = UpdateRoutineRequest,
    responses((status = 200, body = RoutineResponse), (status = 400, description = "Invalid"), (status = 404, description = "Not found")))]
pub async fn update(
    State(store): State<RoutineStore>,
    Path(id): Path<String>,
    Json(body): Json<UpdateRoutineRequest>,
) -> Result<Json<RoutineResponse>, AppError> {
    Ok(Json(svc_update(&store, &id, body)?))
}

/// `PUT /routines/{id}` — fully replace a routine (behaves identically to PATCH).
#[utoipa::path(put, path = "/routines/{id}",
    params(("id" = String, Path, description = "Routine UUID")),
    request_body = UpdateRoutineRequest,
    responses((status = 200, body = RoutineResponse), (status = 400, description = "Invalid"), (status = 404, description = "Not found")))]
pub async fn replace(
    state: State<RoutineStore>,
    path: Path<String>,
    body: Json<UpdateRoutineRequest>,
) -> Result<Json<RoutineResponse>, AppError> {
    update(state, path, body).await
}

/// `DELETE /routines/{id}` — delete a routine by UUID.
#[utoipa::path(delete, path = "/routines/{id}",
    params(("id" = String, Path, description = "Routine UUID")),
    responses((status = 200, body = RoutineResponse), (status = 404, description = "Not found")))]
pub async fn delete(
    State(store): State<RoutineStore>,
    Path(id): Path<String>,
) -> Result<Json<RoutineResponse>, AppError> {
    Ok(Json(svc_delete(&store, &id)?))
}

/// `POST /routines/{id}/trigger` — manually run a routine outside its schedule.
#[utoipa::path(post, path = "/routines/{id}/trigger",
    params(("id" = String, Path, description = "Routine UUID")),
    responses((status = 200, body = Routine), (status = 404, description = "Not found")))]
pub async fn trigger(
    State(store): State<RoutineStore>,
    Path(id): Path<String>,
) -> Result<Json<Routine>, AppError> {
    Ok(Json(svc_trigger(&store, &id)?))
}

/// `GET /routines/{id}/logs` — return the newest workbench `agent.log` as plain text.
#[utoipa::path(get, path = "/routines/{id}/logs",
    params(("id" = String, Path, description = "Routine UUID")),
    responses((status = 200, description = "Log file contents as plain text"), (status = 404, description = "Not found")))]
pub async fn get_logs(
    State(store): State<RoutineStore>,
    Path(id): Path<String>,
) -> Result<String, AppError> {
    svc_logs(&store, &id)
}

#[cfg(test)]
#[path = "routines_tests.rs"]
mod routines_tests;
