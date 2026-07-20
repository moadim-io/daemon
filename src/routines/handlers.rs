//! Axum HTTP handlers for the `/routines` resource.

use axum::{
    extract::{Path, Query, State},
    http::header,
    response::IntoResponse,
    Json,
};
use serde::Deserialize;

use crate::error::{run_blocking, AppError};

use super::ical::{svc_ical, svc_ical_routine};
use super::model::{FleetRunSummary, IcalFeedQuery, Routine, RoutineStore};
use super::service::{
    svc_get_prompt_preview, svc_list_all_runs, svc_logs, svc_run_log, svc_run_summary,
    svc_trigger_scheduled,
};

/// `GET /routines/{id}/prompt-preview` — the exact prompt body a run would receive, computed
/// in-memory with no workbench, `prompt.md` write, or agent launch (issue #391). Does not include
/// the routine-origin disclosure written separately to `CLAUDE.md` at trigger time.
#[utoipa::path(get, path = "/routines/{id}/prompt-preview",
    params(("id" = String, Path, description = "Routine UUID")),
    responses((status = 200, description = "Composed prompt body as plain text"), (status = 404, description = "Not found")))]
pub async fn get_prompt_preview(
    State(store): State<RoutineStore>,
    Path(id): Path<String>,
) -> Result<String, AppError> {
    svc_get_prompt_preview(&store, &id)
}

/// `POST /routines/{id}/scheduled-trigger` — run a routine on its schedule.
///
/// The daemon-side endpoint the generated crontab line invokes (`moadim schedule trigger <id>`).
/// Unlike [`crate::routes::trigger_routine::trigger_routine`] it does not record a manual
/// trigger; the spawned command records the scheduled timestamp itself. See
/// [`svc_trigger_scheduled`].
#[utoipa::path(post, path = "/routines/{id}/scheduled-trigger",
    params(("id" = String, Path, description = "Routine UUID")),
    responses((status = 200, body = Routine), (status = 404, description = "Not found")))]
pub async fn scheduled_trigger(
    State(store): State<RoutineStore>,
    Path(id): Path<String>,
) -> Result<Json<Routine>, AppError> {
    // See `crate::routes::trigger_routine::trigger_routine`: `svc_trigger_scheduled` shells
    // out to `tmux`(1) too (#360). This is the endpoint the generated crontab line invokes, so
    // a `*/N` herd of scheduled fires is exactly the thundering-herd case #360 is about.
    let resp = run_blocking(move || svc_trigger_scheduled(&store, &id)).await?;
    Ok(Json(resp))
}

/// `GET /routines.ics` — iCalendar feed of every enabled routine's upcoming fire times.
///
/// Returns a `text/calendar` body suitable for subscribing to in an external calendar
/// (Google Calendar, Apple Calendar, …) so upcoming runs show up alongside other events.
/// With `?routine=<id>` the feed is scoped to a single routine (named after it); without
/// it every enabled routine is rendered (issue #263).
#[utoipa::path(get, path = "/routines.ics",
    params(IcalFeedQuery),
    responses((status = 200, description = "iCalendar (text/calendar) feed of upcoming routine fire times")))]
pub async fn ical_feed(
    State(state): State<crate::routes::http::AppState>,
    Query(query): Query<IcalFeedQuery>,
) -> impl IntoResponse {
    let body = match query.routine.as_deref() {
        Some(id) => svc_ical_routine(&state.routines, &state.routines_dir, id),
        None => svc_ical(&state.routines, &state.routines_dir),
    };
    (
        [(header::CONTENT_TYPE, "text/calendar; charset=utf-8")],
        body,
    )
}

/// `GET /routines/{id}/logs` — return the newest workbench `agent.log` as plain text.
#[utoipa::path(get, path = "/routines/{id}/logs",
    params(("id" = String, Path, description = "Routine UUID")),
    responses((status = 200, description = "Log file contents as plain text"), (status = 404, description = "Not found")))]
pub async fn get_logs(
    State(store): State<RoutineStore>,
    Path(id): Path<String>,
) -> Result<String, AppError> {
    svc_logs(&store, &id).map(|logs| logs.content)
}

/// Query parameters for `GET /routines/runs`.
#[derive(Deserialize, utoipa::IntoParams)]
pub struct FleetRunsQuery {
    /// Cap on the number of runs returned (default: `DEFAULT_FLEET_RUNS_LIMIT`).
    pub limit: Option<usize>,
}

/// `GET /routines/runs` — the most recent runs across every routine, newest first. Backs the
/// overview "recent runs" panel with a single workbench scan instead of one request per routine.
#[utoipa::path(get, path = "/routines/runs",
    params(FleetRunsQuery),
    responses((status = 200, body = [FleetRunSummary])))]
pub async fn get_all_runs(
    State(store): State<RoutineStore>,
    Query(query): Query<FleetRunsQuery>,
) -> Json<Vec<FleetRunSummary>> {
    Json(svc_list_all_runs(&store, query.limit))
}

/// `GET /routines/{id}/runs/{workbench}/log` — return one specific run's `agent.log` as plain text.
#[utoipa::path(get, path = "/routines/{id}/runs/{workbench}/log",
    params(
        ("id" = String, Path, description = "Routine UUID"),
        ("workbench" = String, Path, description = "Workbench directory name (`{slug}-{unix_secs}`), from `GET /routines/{id}/runs`"),
    ),
    responses((status = 200, description = "Log file contents as plain text"), (status = 404, description = "Not found")))]
pub async fn get_run_log(
    State(store): State<RoutineStore>,
    Path((id, workbench)): Path<(String, String)>,
) -> Result<String, AppError> {
    svc_run_log(&store, &id, &workbench)
}

/// `GET /routines/{id}/runs/{workbench}/summary` — return one specific run's agent-authored
/// `summary.md` as plain text (empty when the agent hasn't written one).
#[utoipa::path(get, path = "/routines/{id}/runs/{workbench}/summary",
    params(
        ("id" = String, Path, description = "Routine UUID"),
        ("workbench" = String, Path, description = "Workbench directory name (`{slug}-{unix_secs}`), from `GET /routines/{id}/runs`"),
    ),
    responses((status = 200, description = "Summary file contents as plain text"), (status = 404, description = "Not found")))]
pub async fn get_run_summary(
    State(store): State<RoutineStore>,
    Path((id, workbench)): Path<(String, String)>,
) -> Result<String, AppError> {
    svc_run_summary(&store, &id, &workbench)
}
