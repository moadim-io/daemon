//! Cron job model and service functions that proxy the OS user crontab.
//!
//! Managed entries are tagged with a `# moadim-id: <uuid>` comment on the
//! line immediately preceding their crontab entry. Untagged entries are
//! exposed read-only with `source = "system"`.

use axum::{
    extract::Path,
    http::StatusCode,
    Json,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::process::{Command, Stdio};
use uuid::Uuid;

use crate::error::AppError;

const MOADIM_TAG: &str = "# moadim-id:";

/// A cron entry from the user crontab.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, utoipa::ToSchema)]
pub struct CronJob {
    /// Stable identifier. UUID for managed entries; deterministic hash for system entries.
    pub id: String,
    /// Cron schedule expression (5-field or `@keyword`).
    pub schedule: String,
    /// The command the OS executes.
    pub command: String,
    /// `"managed"` for entries owned by this server; `"system"` for pre-existing entries.
    pub source: String,
}

/// Request body for creating a new cron job.
#[derive(Deserialize, JsonSchema, utoipa::ToSchema)]
pub struct CreateRequest {
    /// 5-field cron expression (`min hour dom month dow`) or `@keyword` (`@daily`, `@hourly`, etc.).
    pub schedule: String,
    /// The command the OS will execute.
    pub command: String,
}

/// Request body for partially updating an existing managed cron job.
#[derive(Deserialize, JsonSchema, utoipa::ToSchema)]
pub struct UpdateRequest {
    /// New schedule, or `None` to keep existing.
    pub schedule: Option<String>,
    /// New command, or `None` to keep existing.
    pub command: Option<String>,
}

// --- Crontab I/O ---

fn read_crontab() -> Result<String, AppError> {
    let out = Command::new("crontab")
        .arg("-l")
        .output()
        .map_err(|_| AppError::Internal)?;
    // "no crontab for user" exits non-zero with message on stderr; stdout is empty
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

fn write_crontab(content: &str) -> Result<(), AppError> {
    let mut child = Command::new("crontab")
        .arg("-")
        .stdin(Stdio::piped())
        .spawn()
        .map_err(|_| AppError::Internal)?;
    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(content.as_bytes())
        .map_err(|_| AppError::Internal)?;
    child.wait().map_err(|_| AppError::Internal)?;
    Ok(())
}

// --- Parsing ---

fn is_env_var(line: &str) -> bool {
    if let Some(eq) = line.find('=') {
        let key = &line[..eq];
        !key.is_empty() && key.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
    } else {
        false
    }
}

fn parse_cron_line(line: &str) -> Option<(String, String)> {
    if let Some(rest) = line.strip_prefix('@') {
        let kw_end = rest
            .find(|c: char| c.is_ascii_whitespace())
            .unwrap_or(rest.len());
        let keyword = &rest[..kw_end];
        let command = rest[kw_end..].trim_start();
        if command.is_empty() {
            return None;
        }
        Some((format!("@{}", keyword), command.to_string()))
    } else {
        let tokens: Vec<&str> = line.split_ascii_whitespace().collect();
        if tokens.len() < 6 {
            return None;
        }
        let schedule = tokens[..5].join(" ");
        let command = tokens[5..].join(" ");
        Some((schedule, command))
    }
}

fn stable_id(schedule: &str, command: &str) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut h = DefaultHasher::new();
    schedule.hash(&mut h);
    command.hash(&mut h);
    format!("sys-{:016x}", h.finish())
}

fn parse_crontab(text: &str) -> Vec<CronJob> {
    let mut jobs = Vec::new();
    let mut pending_id: Option<String> = None;

    for line in text.lines() {
        let trimmed = line.trim();

        if let Some(id_part) = trimmed.strip_prefix(MOADIM_TAG) {
            pending_id = Some(id_part.trim().to_string());
            continue;
        }

        if trimmed.is_empty() || trimmed.starts_with('#') || is_env_var(trimmed) {
            pending_id = None;
            continue;
        }

        if let Some((schedule, command)) = parse_cron_line(trimmed) {
            let (id, source) = match pending_id.take() {
                Some(mid) => (mid, "managed".to_string()),
                None => (stable_id(&schedule, &command), "system".to_string()),
            };
            jobs.push(CronJob {
                id,
                schedule,
                command,
                source,
            });
        } else {
            pending_id = None;
        }
    }
    jobs
}

fn validate_schedule(expr: &str) -> Result<(), AppError> {
    let e = expr.trim();
    if e.starts_with('@') || e.split_ascii_whitespace().count() == 5 {
        return Ok(());
    }
    Err(AppError::BadRequest(
        "invalid cron expression: expected 5 fields or @keyword".to_string(),
    ))
}

// --- Service layer ---

/// Return all cron jobs from the user crontab (managed + system).
pub fn svc_list() -> Result<Vec<CronJob>, AppError> {
    Ok(parse_crontab(&read_crontab()?))
}

/// Look up a single job by ID.
pub fn svc_get(id: &str) -> Result<CronJob, AppError> {
    svc_list()?
        .into_iter()
        .find(|j| j.id == id)
        .ok_or(AppError::NotFound)
}

/// Append a new managed entry to the user crontab.
pub fn svc_create(req: CreateRequest) -> Result<CronJob, AppError> {
    validate_schedule(&req.schedule)?;
    let id = Uuid::new_v4().to_string();
    let mut text = read_crontab()?;
    if !text.is_empty() && !text.ends_with('\n') {
        text.push('\n');
    }
    text.push_str(&format!(
        "{} {}\n{} {}\n",
        MOADIM_TAG, id, req.schedule, req.command
    ));
    write_crontab(&text)?;
    Ok(CronJob {
        id,
        schedule: req.schedule,
        command: req.command,
        source: "managed".to_string(),
    })
}

/// Update schedule and/or command of a managed entry in place.
pub fn svc_update(id: &str, req: UpdateRequest) -> Result<CronJob, AppError> {
    if let Some(ref s) = req.schedule {
        validate_schedule(s)?;
    }
    let text = read_crontab()?;
    let tag = format!("{} {}", MOADIM_TAG, id);
    let lines: Vec<&str> = text.lines().collect();
    let pos = lines
        .iter()
        .position(|l| l.trim() == tag.trim())
        .ok_or(AppError::NotFound)?;
    if pos + 1 >= lines.len() {
        return Err(AppError::NotFound);
    }
    let old_cron = lines[pos + 1];
    let (old_schedule, old_command) =
        parse_cron_line(old_cron.trim()).ok_or(AppError::NotFound)?;
    let new_schedule = req.schedule.unwrap_or(old_schedule);
    let new_command = req.command.unwrap_or(old_command);
    let new_cron_line = format!("{} {}", new_schedule, new_command);
    let new_lines: Vec<&str> = lines.clone();
    // replace only the cron line; the tag line stays
    let new_cron_owned = new_cron_line.clone();
    let mut result_lines: Vec<String> = lines
        .iter()
        .enumerate()
        .map(|(i, l)| {
            if i == pos + 1 {
                new_cron_owned.clone()
            } else {
                l.to_string()
            }
        })
        .collect();
    result_lines.push(String::new()); // trailing newline
    write_crontab(&result_lines.join("\n"))?;
    drop(new_lines);
    Ok(CronJob {
        id: id.to_string(),
        schedule: new_schedule,
        command: new_command,
        source: "managed".to_string(),
    })
}

/// Remove a managed entry (tag line + cron line) from the user crontab.
pub fn svc_delete(id: &str) -> Result<CronJob, AppError> {
    let text = read_crontab()?;
    let tag = format!("{} {}", MOADIM_TAG, id);
    let lines: Vec<&str> = text.lines().collect();
    let pos = lines
        .iter()
        .position(|l| l.trim() == tag.trim())
        .ok_or(AppError::NotFound)?;
    if pos + 1 >= lines.len() {
        return Err(AppError::NotFound);
    }
    let (schedule, command) =
        parse_cron_line(lines[pos + 1].trim()).ok_or(AppError::NotFound)?;
    let new_lines: Vec<String> = lines
        .iter()
        .enumerate()
        .filter(|(i, _)| *i != pos && *i != pos + 1)
        .map(|(_, l)| l.to_string())
        .collect();
    let mut out = new_lines.join("\n");
    if !out.is_empty() && !out.ends_with('\n') {
        out.push('\n');
    }
    write_crontab(&out)?;
    Ok(CronJob {
        id: id.to_string(),
        schedule,
        command,
        source: "managed".to_string(),
    })
}

// --- Axum HTTP handlers ---

/// `GET /cron-jobs` — list all cron jobs (managed and system).
#[utoipa::path(get, path = "/cron-jobs",
    responses((status = 200, body = Vec<CronJob>)))]
pub async fn list() -> Result<Json<Vec<CronJob>>, AppError> {
    Ok(Json(svc_list()?))
}

/// `POST /cron-jobs` — add a new managed cron job.
#[utoipa::path(post, path = "/cron-jobs",
    request_body = CreateRequest,
    responses((status = 201, body = CronJob), (status = 400, description = "Invalid schedule")))]
pub async fn create(
    Json(body): Json<CreateRequest>,
) -> Result<(StatusCode, Json<CronJob>), AppError> {
    Ok((StatusCode::CREATED, Json(svc_create(body)?)))
}

/// `GET /cron-jobs/{id}` — retrieve a single cron job by ID.
#[utoipa::path(get, path = "/cron-jobs/{id}",
    params(("id" = String, Path, description = "Cron job ID")),
    responses((status = 200, body = CronJob), (status = 404, description = "Not found")))]
pub async fn get(Path(id): Path<String>) -> Result<Json<CronJob>, AppError> {
    Ok(Json(svc_get(&id)?))
}

/// `PATCH /cron-jobs/{id}` — partially update a managed cron job.
#[utoipa::path(patch, path = "/cron-jobs/{id}",
    params(("id" = String, Path, description = "Cron job ID")),
    request_body = UpdateRequest,
    responses((status = 200, body = CronJob), (status = 400, description = "Invalid"), (status = 404, description = "Not found")))]
pub async fn update(
    Path(id): Path<String>,
    Json(body): Json<UpdateRequest>,
) -> Result<Json<CronJob>, AppError> {
    Ok(Json(svc_update(&id, body)?))
}

/// `DELETE /cron-jobs/{id}` — remove a managed cron job.
#[utoipa::path(delete, path = "/cron-jobs/{id}",
    params(("id" = String, Path, description = "Cron job ID")),
    responses((status = 200, body = CronJob), (status = 404, description = "Not found")))]
pub async fn delete(Path(id): Path<String>) -> Result<Json<CronJob>, AppError> {
    Ok(Json(svc_delete(&id)?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_schedule_accepts_five_field() {
        assert!(validate_schedule("30 9 * * 1-5").is_ok());
        assert!(validate_schedule("* * * * *").is_ok());
    }

    #[test]
    fn validate_schedule_accepts_at_keywords() {
        assert!(validate_schedule("@daily").is_ok());
        assert!(validate_schedule("@hourly").is_ok());
        assert!(validate_schedule("@reboot").is_ok());
    }

    #[test]
    fn validate_schedule_rejects_six_field() {
        assert!(validate_schedule("0 30 9 * * 1-5").is_err());
        assert!(validate_schedule("not a cron").is_err());
    }

    #[test]
    fn parse_managed_entry() {
        let text = "# moadim-id: abc-123\n30 9 * * 1-5 /usr/bin/backup.sh\n";
        let jobs = parse_crontab(text);
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].id, "abc-123");
        assert_eq!(jobs[0].schedule, "30 9 * * 1-5");
        assert_eq!(jobs[0].command, "/usr/bin/backup.sh");
        assert_eq!(jobs[0].source, "managed");
    }

    #[test]
    fn parse_system_entry() {
        let text = "* * * * * /usr/bin/ping google.com\n";
        let jobs = parse_crontab(text);
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].source, "system");
        assert!(jobs[0].id.starts_with("sys-"));
    }

    #[test]
    fn parse_at_keyword_managed() {
        let text = "# moadim-id: xyz\n@daily /usr/bin/cleanup.sh\n";
        let jobs = parse_crontab(text);
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].schedule, "@daily");
        assert_eq!(jobs[0].source, "managed");
    }

    #[test]
    fn parse_mixed_entries() {
        let text = concat!(
            "# moadim-id: aaa\n",
            "0 1 * * * /managed-cmd\n",
            "30 9 * * * /system-cmd\n",
        );
        let jobs = parse_crontab(text);
        assert_eq!(jobs.len(), 2);
        assert_eq!(jobs[0].source, "managed");
        assert_eq!(jobs[1].source, "system");
    }

    #[test]
    fn parse_skips_blanks_and_comments() {
        let text = "# a comment\n\nMAILTO=\"\"\n* * * * * /cmd\n";
        let jobs = parse_crontab(text);
        assert_eq!(jobs.len(), 1);
    }

    #[test]
    fn stable_id_is_deterministic() {
        let a = stable_id("* * * * *", "/cmd");
        let b = stable_id("* * * * *", "/cmd");
        assert_eq!(a, b);
        assert!(a.starts_with("sys-"));
    }
}
