//! Read-only discovery of system cron jobs from crontab and `/etc/cron.d`.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::process::Command;

use crate::cron_jobs::CronJob;

/// Return all system cron jobs found across user crontab and `/etc/cron*` paths.
pub fn read_all() -> Vec<CronJob> {
    let mut jobs = Vec::new();
    jobs.extend(read_user_crontab());
    jobs.extend(read_etc_crontab());
    jobs.extend(read_cron_d());
    jobs
}

/// Parse bytes from a crontab command's stdout into cron jobs.
fn parse_crontab_output(stdout: &[u8], source: &str) -> Vec<CronJob> {
    let text = String::from_utf8_lossy(stdout);
    parse_text(&text, source, false)
}

/// Read a crontab-format file at `path` and return parsed jobs, or empty on error.
fn read_crontab_from_path(
    path: &std::path::Path,
    source: &str,
    has_user_field: bool,
) -> Vec<CronJob> {
    match std::fs::read_to_string(path) {
        Ok(text) => parse_text(&text, source, has_user_field),
        Err(_) => vec![],
    }
}

/// Scan all files in `dir` as cron.d-style entries, returning parsed jobs.
fn read_cron_d_from_dir(dir: &std::path::Path) -> Vec<CronJob> {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return vec![],
    };
    let mut jobs = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();
        let source = format!("system:cron.d/{}", name);
        if let Ok(text) = std::fs::read_to_string(&path) {
            jobs.extend(parse_text(&text, &source, true));
        }
    }
    jobs
}

/// Parse jobs from `crontab -l` output of the current user.
fn read_user_crontab() -> Vec<CronJob> {
    let output = match Command::new("crontab").arg("-l").output() {
        Ok(o) if o.status.success() => o,
        _ => return vec![],
    };
    parse_crontab_output(&output.stdout, "system:user-crontab")
}

/// Parse jobs from `/etc/crontab` if it exists.
fn read_etc_crontab() -> Vec<CronJob> {
    read_crontab_from_path(
        std::path::Path::new("/etc/crontab"),
        "system:etc-crontab",
        true,
    )
}

/// Parse jobs from all files under `/etc/cron.d/`.
fn read_cron_d() -> Vec<CronJob> {
    read_cron_d_from_dir(std::path::Path::new("/etc/cron.d"))
}

/// Produce a deterministic ID from `(source, schedule, command)` so system jobs have stable IDs across reads.
fn stable_id(source: &str, schedule: &str, command: &str) -> String {
    let mut h = DefaultHasher::new();
    source.hash(&mut h);
    schedule.hash(&mut h);
    command.hash(&mut h);
    format!("sys-{:016x}", h.finish())
}

/// Return `true` if `line` looks like a shell variable assignment (`KEY=value`).
fn is_env_var_line(line: &str) -> bool {
    if let Some(eq_pos) = line.find('=') {
        let key = &line[..eq_pos];
        !key.is_empty() && key.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
    } else {
        false
    }
}

/// Parse every line in `text` into cron jobs, skipping blanks and comments.
fn parse_text(text: &str, source: &str, has_user_field: bool) -> Vec<CronJob> {
    text.lines()
        .filter_map(|line| parse_line(line, source, has_user_field))
        .collect()
}

/// Parse a single crontab line into a [`CronJob`], returning `None` for non-job lines.
fn parse_line(line: &str, source: &str, has_user_field: bool) -> Option<CronJob> {
    let line = line.trim();
    if line.is_empty() || line.starts_with('#') || is_env_var_line(line) {
        return None;
    }

    let (schedule, command) = if let Some(rest) = line.strip_prefix('@') {
        // @reboot, @daily, @weekly, @monthly, @yearly, @hourly, etc.
        let kw_end = rest
            .find(|c: char| c.is_ascii_whitespace())
            .unwrap_or(rest.len());
        let keyword = &rest[..kw_end];
        let after = rest[kw_end..].trim_start();
        let cmd = if has_user_field {
            let user_end = after
                .find(|c: char| c.is_ascii_whitespace())
                .unwrap_or(after.len());
            after[user_end..].trim_start()
        } else {
            after
        };
        if cmd.is_empty() {
            return None;
        }
        (format!("@{}", keyword), cmd.to_string())
    } else {
        // Standard: min hour dom month dow [user] command
        let tokens: Vec<&str> = line.split_ascii_whitespace().collect();
        let min_fields = if has_user_field { 7 } else { 6 };
        if tokens.len() < min_fields {
            return None;
        }
        let schedule = tokens[..5].join(" ");
        let cmd_start = if has_user_field { 6 } else { 5 };
        let command = tokens[cmd_start..].join(" ");
        (schedule, command)
    };

    Some(CronJob {
        id: stable_id(source, &schedule, &command),
        schedule,
        handler: command,
        metadata: serde_json::json!({}),
        enabled: true,
        source: source.to_string(),
        created_at: 0,
        updated_at: 0,
        last_triggered_at: None,
    })
}

#[cfg(test)]
#[path = "system_cron_tests.rs"]
mod system_cron_tests;
