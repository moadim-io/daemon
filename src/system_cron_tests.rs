#![allow(clippy::missing_docs_in_private_items)]

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

#[test]
fn stable_id_differs_for_different_inputs() {
    let id1 = stable_id("src-a", "@daily", "/bin/a");
    let id2 = stable_id("src-b", "@daily", "/bin/a");
    assert_ne!(id1, id2);
}

#[test]
fn is_env_var_line_detects_assignment() {
    assert!(is_env_var_line("MAILTO=\"\""));
    assert!(is_env_var_line("PATH=/usr/bin"));
    assert!(is_env_var_line("FOO_BAR=baz"));
}

#[test]
fn is_env_var_line_ignores_non_assignment() {
    assert!(!is_env_var_line("30 9 * * * /bin/cmd"));
    assert!(!is_env_var_line("@daily /bin/cmd"));
    assert!(!is_env_var_line("# comment"));
}

#[test]
fn parse_text_handles_multiple_lines() {
    let text = "# header\n30 9 * * 1-5 /bin/a\n@daily /bin/b\n";
    let jobs = parse_text(text, "src", false);
    assert_eq!(jobs.len(), 2);
}

#[test]
fn parse_line_with_too_few_fields_returns_none() {
    assert!(parse_line("* * * * /bin/cmd", "test", false).is_none());
}

#[test]
fn parse_line_at_syntax_without_command_returns_none() {
    assert!(parse_line("@daily", "test", false).is_none());
}

#[test]
fn parsed_job_has_managed_false_source_field_propagated() {
    let job = parse_line("* * * * * /bin/cmd", "my-source", false).unwrap();
    assert_eq!(job.source, "my-source");
    assert!(job.enabled);
    assert_eq!(job.created_at, 0);
}

#[test]
fn parse_crontab_output_parses_bytes() {
    let jobs = parse_crontab_output(b"30 9 * * 1-5 /bin/backup\n", "src");
    assert_eq!(jobs.len(), 1);
    assert_eq!(jobs[0].schedule, "30 9 * * 1-5");
}

#[test]
fn read_crontab_from_path_reads_valid_file() {
    let dir = std::env::temp_dir();
    let path = dir.join("test-crontab-coverage");
    std::fs::write(&path, "30 9 * * 1-5 /bin/backup\n").unwrap();
    let jobs = read_crontab_from_path(&path, "test", false);
    assert_eq!(jobs.len(), 1);
    std::fs::remove_file(&path).unwrap();
}

#[test]
fn read_crontab_from_path_missing_file_returns_empty() {
    let jobs = read_crontab_from_path(
        std::path::Path::new("/nonexistent-crontab-9999"),
        "t",
        false,
    );
    assert!(jobs.is_empty());
}

#[test]
fn read_cron_d_from_dir_reads_cron_files() {
    let dir = std::env::temp_dir().join("test-cron-d-coverage");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("my-job"), "* * * * * root /bin/cmd\n").unwrap();
    let jobs = read_cron_d_from_dir(&dir);
    assert!(!jobs.is_empty());
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn read_cron_d_from_dir_missing_returns_empty() {
    let jobs = read_cron_d_from_dir(std::path::Path::new("/nonexistent-cron-d-9999"));
    assert!(jobs.is_empty());
}

#[test]
fn read_cron_d_from_dir_skips_subdirectories() {
    let dir = std::env::temp_dir().join("test-cron-d-subdir-coverage");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::create_dir_all(dir.join("subdir")).unwrap();
    let jobs = read_cron_d_from_dir(&dir);
    assert!(jobs.is_empty());
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
#[cfg(unix)]
fn read_user_crontab_success_path() {
    use std::os::unix::fs::PermissionsExt;

    let tmp = std::env::temp_dir().join("fake-crontab-for-coverage");
    std::fs::create_dir_all(&tmp).unwrap();
    let script = tmp.join("crontab");
    // Script that succeeds with one cron job line
    std::fs::write(&script, "#!/bin/sh\necho '* * * * * /bin/cmd'\n").unwrap();
    let mut perms = std::fs::metadata(&script).unwrap().permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(&script, perms).unwrap();

    let original_path = std::env::var("PATH").unwrap_or_default();
    // SAFETY: tests are single-threaded by default; PATH is restored immediately after.
    unsafe {
        std::env::set_var("PATH", format!("{}:{}", tmp.display(), original_path));
    }

    let jobs = read_user_crontab();

    unsafe {
        std::env::set_var("PATH", &original_path);
    }
    std::fs::remove_dir_all(&tmp).unwrap();

    assert_eq!(jobs.len(), 1);
    assert_eq!(jobs[0].schedule, "* * * * *");
}
