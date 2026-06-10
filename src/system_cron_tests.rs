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
