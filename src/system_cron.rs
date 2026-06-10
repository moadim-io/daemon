//! Read-only discovery of system cron jobs from crontab and `/etc/cron.d`.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::Path;
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

/// Parse jobs from `crontab -l` output of the current user.
fn read_user_crontab() -> Vec<CronJob> {
    let output = match Command::new("crontab").arg("-l").output() {
        Ok(o) if o.status.success() => o,
        _ => return vec![],
    };
    let text = String::from_utf8_lossy(&output.stdout);
    parse_text(&text, "system:user-crontab", false)
}

/// Parse jobs from `/etc/crontab` if it exists.
fn read_etc_crontab() -> Vec<CronJob> {
    let path = Path::new("/etc/crontab");
    if !path.exists() {
        return vec![];
    }
    match std::fs::read_to_string(path) {
        Ok(text) => parse_text(&text, "system:etc-crontab", true),
        Err(_) => vec![],
    }
}

/// Parse jobs from all files under `/etc/cron.d/`.
fn read_cron_d() -> Vec<CronJob> {
    let dir = Path::new("/etc/cron.d");
    if !dir.is_dir() {
        return vec![];
    }
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
mod tests {
    use super::*;

    #[test]
    fn parses_standard_line() {
        let job = parse_line("30 9 * * 1-5 /usr/bin/backup.sh", "test", false).unwrap();
        assert_eq!(job.schedule, "30 9 * * 1-5");
        assert_eq!(job.handler, "/usr/bin/backup.sh");
        assert_eq!(job.source, "test");
    }

    #[test]
    fn parses_at_syntax() {
        let job = parse_line("@daily /usr/bin/cleanup.sh", "test", false).unwrap();
        assert_eq!(job.schedule, "@daily");
        assert_eq!(job.handler, "/usr/bin/cleanup.sh");
    }

    #[test]
    fn parses_etc_crontab_with_user() {
        let job = parse_line("* * * * * root /usr/sbin/ntpdate", "etc", true).unwrap();
        assert_eq!(job.schedule, "* * * * *");
        assert_eq!(job.handler, "/usr/sbin/ntpdate");
    }

    #[test]
    fn parses_at_syntax_with_user() {
        let job = parse_line("@reboot root /usr/sbin/cron-startup", "etc", true).unwrap();
        assert_eq!(job.schedule, "@reboot");
        assert_eq!(job.handler, "/usr/sbin/cron-startup");
    }

    #[test]
    fn skips_comments() {
        assert!(parse_line("# this is a comment", "test", false).is_none());
    }

    #[test]
    fn skips_env_vars() {
        assert!(parse_line("MAILTO=\"\"", "test", false).is_none());
        assert!(parse_line("PATH=/usr/bin:/usr/sbin", "test", false).is_none());
    }

    #[test]
    fn skips_blank_lines() {
        assert!(parse_line("   ", "test", false).is_none());
        assert!(parse_line("", "test", false).is_none());
    }

    #[test]
    fn stable_id_is_deterministic() {
        let id1 = stable_id("system:user-crontab", "@daily", "/usr/bin/backup.sh");
        let id2 = stable_id("system:user-crontab", "@daily", "/usr/bin/backup.sh");
        assert_eq!(id1, id2);
        assert!(id1.starts_with("sys-"));
    }
}
