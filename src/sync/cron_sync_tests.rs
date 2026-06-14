#![allow(clippy::missing_docs_in_private_items)]

use std::path::Path;

use super::*;

// ─── Schedule conversion ───────────────────────────────────────────────────

#[test]
fn to_os_schedule_7field_drops_sec_and_year() {
    assert_eq!(to_os_schedule("0 30 9 * * 1-5 *"), "30 9 * * 1-5");
}

#[test]
fn to_os_schedule_passthrough_keyword() {
    assert_eq!(to_os_schedule("@daily"), "@daily");
    assert_eq!(to_os_schedule("@reboot"), "@reboot");
    assert_eq!(to_os_schedule("@hourly"), "@hourly");
}

#[test]
fn to_os_schedule_5field_unchanged() {
    assert_eq!(to_os_schedule("30 9 * * 1-5"), "30 9 * * 1-5");
}

#[test]
fn to_os_schedule_trims_whitespace() {
    assert_eq!(to_os_schedule("  0 0 * * * * *  "), "0 * * * *");
}

#[test]
fn to_moadim_schedule_5field_passthrough() {
    assert_eq!(to_moadim_schedule("30 9 * * 1-5"), "30 9 * * 1-5");
}

#[test]
fn to_moadim_schedule_passthrough_keyword() {
    assert_eq!(to_moadim_schedule("@daily"), "@daily");
    assert_eq!(to_moadim_schedule("@hourly"), "@hourly");
}

#[test]
fn to_moadim_schedule_trims_whitespace() {
    assert_eq!(to_moadim_schedule("  30 9 * * 1-5  "), "30 9 * * 1-5");
}

#[test]
fn roundtrip_5field_via_os() {
    let moadim = "30 9 * * 1-5";
    let os = to_os_schedule(moadim);
    assert_eq!(to_moadim_schedule(&os), "30 9 * * 1-5");
}

#[test]
fn roundtrip_keyword_unchanged() {
    let moadim = "@weekly";
    assert_eq!(to_moadim_schedule(&to_os_schedule(moadim)), "@weekly");
}

// ─── Handler path resolution ───────────────────────────────────────────────

#[test]
fn resolve_handler_path_returns_name_when_not_found() {
    let dir = Path::new("/nonexistent/handlers");
    let result = resolve_handler_path("my-script", dir);
    assert_eq!(result, dir.join("my-script"));
}

#[test]
fn handler_from_command_strips_prefix_and_extension() {
    let dir = Path::new("/home/user/.config/moadim/handlers");
    let cmd = "/home/user/.config/moadim/handlers/send-report.sh";
    assert_eq!(handler_from_command(cmd, dir), "send-report");
}

#[test]
fn handler_from_command_no_extension() {
    let dir = Path::new("/home/user/.config/moadim/handlers");
    let cmd = "/home/user/.config/moadim/handlers/cleanup";
    assert_eq!(handler_from_command(cmd, dir), "cleanup");
}

#[test]
fn handler_from_command_outside_dir_uses_stem() {
    let dir = Path::new("/home/user/.config/moadim/handlers");
    let cmd = "/usr/local/bin/my-script.sh";
    assert_eq!(handler_from_command(cmd, dir), "my-script");
}

// ─── format_crontab_line ───────────────────────────────────────────────────

fn make_job(id: &str, schedule: &str, handler: &str) -> CronJob {
    CronJob {
        id: id.to_string(),
        schedule: schedule.to_string(),
        handler: handler.to_string(),
        metadata: serde_json::json!({}),
        enabled: true,
        source: "managed".to_string(),
        created_at: 0,
        updated_at: 0,
        last_triggered_at: None,
    }
}

#[test]
fn format_crontab_line_produces_moadim_tag() {
    let job = make_job("abc-123", "30 9 * * 1-5", "send-report");
    let dir = Path::new("/home/user/.config/moadim/handlers");
    let line = format_crontab_line(&job, dir);
    assert!(line.contains("# moadim:abc-123"), "missing tag: {line}");
    assert!(line.starts_with("30 9 * * 1-5"), "wrong schedule: {line}");
    assert!(line.contains("send-report"), "missing handler: {line}");
}

#[test]
fn format_crontab_line_keyword_passthrough() {
    let job = make_job("uid-1", "@daily", "backup");
    let dir = Path::new("/home/user/.config/moadim/handlers");
    let line = format_crontab_line(&job, dir);
    assert!(line.starts_with("@daily"), "wrong schedule: {line}");
}

// ─── parse_moadim_line ─────────────────────────────────────────────────────

#[test]
fn parse_moadim_line_standard() {
    let line = "30 9 * * 1-5 /home/user/.config/moadim/handlers/send-report # moadim:abc-123";
    let (id, sched, cmd) = parse_moadim_line(line).unwrap();
    assert_eq!(id, "abc-123");
    assert_eq!(sched, "30 9 * * 1-5");
    assert_eq!(cmd, "/home/user/.config/moadim/handlers/send-report");
}

#[test]
fn parse_moadim_line_keyword() {
    let line = "@daily /home/user/.config/moadim/handlers/backup # moadim:xyz-789";
    let (id, sched, cmd) = parse_moadim_line(line).unwrap();
    assert_eq!(id, "xyz-789");
    assert_eq!(sched, "@daily");
    assert_eq!(cmd, "/home/user/.config/moadim/handlers/backup");
}

#[test]
fn parse_moadim_line_no_tag_returns_none() {
    assert!(parse_moadim_line("30 9 * * 1-5 /usr/bin/cmd").is_none());
}

#[test]
fn parse_moadim_line_empty_uuid_returns_none() {
    assert!(parse_moadim_line("30 9 * * 1-5 /cmd # moadim:").is_none());
}

#[test]
fn parse_moadim_line_too_few_fields_returns_none() {
    assert!(parse_moadim_line("30 9 /cmd # moadim:uid").is_none());
}

// ─── parse_block ───────────────────────────────────────────────────────────

#[test]
fn parse_block_extracts_entries() {
    let crontab = "
0 0 * * * /bin/other
# BEGIN MOADIM
# Managed by moadim
30 9 * * 1-5 /handlers/send # moadim:uuid-1
0 0 * * 0 /handlers/clean # moadim:uuid-2
# END MOADIM
@reboot /bin/startup
";
    let entries = parse_block(crontab);
    assert_eq!(entries.len(), 2);
    assert!(entries.contains_key("uuid-1"));
    assert!(entries.contains_key("uuid-2"));
}

#[test]
fn parse_block_empty_when_no_block() {
    assert!(parse_block("0 * * * * /bin/cmd").is_empty());
}

#[test]
fn parse_block_ignores_comment_lines_inside_block() {
    let crontab = "# BEGIN MOADIM\n# a comment\n30 9 * * * /cmd # moadim:id1\n# END MOADIM\n";
    let entries = parse_block(crontab);
    assert_eq!(entries.len(), 1);
    assert!(entries.contains_key("id1"));
}

// ─── replace_block ─────────────────────────────────────────────────────────

#[test]
fn replace_block_inserts_when_absent() {
    let crontab = "0 * * * * /existing\n";
    let block = "# BEGIN MOADIM\n# hdr\n# END MOADIM";
    let result = replace_block(crontab, block);
    assert!(result.contains(BLOCK_BEGIN));
    assert!(result.contains("# END MOADIM"));
    assert!(result.contains("/existing"));
}

#[test]
fn replace_block_replaces_existing() {
    let crontab = "before\n# BEGIN MOADIM\nold line # moadim:old\n# END MOADIM\nafter\n";
    let block = "# BEGIN MOADIM\nnew line # moadim:new\n# END MOADIM";
    let result = replace_block(crontab, block);
    assert!(result.contains("new line"), "new line missing: {result}");
    assert!(!result.contains("old line"), "old line still present: {result}");
    assert!(result.contains("before"), "before missing: {result}");
    assert!(result.contains("after"), "after missing: {result}");
}

#[test]
fn replace_block_idempotent() {
    let block = "# BEGIN MOADIM\n# hdr\n30 9 * * * /cmd # moadim:uid\n# END MOADIM";
    let crontab = format!("{block}\n");
    let result = replace_block(&crontab, block);
    // Content identical (modulo trailing newline normalisation)
    assert!(result.contains("30 9 * * * /cmd # moadim:uid"));
}

#[test]
fn replace_block_handles_malformed_missing_end() {
    let crontab = "pre\n# BEGIN MOADIM\norphan line\n";
    let block = "# BEGIN MOADIM\n# hdr\n# END MOADIM";
    let result = replace_block(crontab, block);
    assert!(result.contains("# END MOADIM"), "end marker missing: {result}");
    assert!(!result.contains("orphan"), "orphan line still present: {result}");
    assert!(result.contains("pre"), "pre-content missing: {result}");
}

#[test]
fn replace_block_empty_crontab() {
    let block = "# BEGIN MOADIM\n# hdr\n# END MOADIM";
    let result = replace_block("", block);
    assert_eq!(result.trim(), block.trim());
}
