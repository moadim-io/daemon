#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "tests assert filesystem and process fixtures directly"
)]

use super::super::{
    global_config, global_public_base_url, notify_finished_run, run_url, FailureNotificationConfig,
    RunStatus, CURL_BIN_ENV,
};
use super::{temp_dir, EnvGuard};

fn write_global(toml: &str) {
    std::fs::create_dir_all(crate::paths::config_dir()).unwrap();
    std::fs::write(crate::paths::notifications_toml_path(), toml).unwrap();
}

fn wait_for(path: &std::path::Path, done: impl Fn(&str) -> bool) -> String {
    for _ in 0..500 {
        let text = std::fs::read_to_string(path).unwrap_or_default();
        if done(&text) {
            return text;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    panic!("{} was never written", path.display());
}

#[test]
fn run_url_defaults_to_the_bind_address() {
    let _addr = EnvGuard::set("MOADIM_BIND_ADDR", "127.0.0.1:6123");
    let expected = "http://127.0.0.1:6123/runs/r1/run-1";
    assert_eq!(run_url(None, "r1", "run-1"), expected);
    assert_eq!(run_url(Some("  "), "r1", "run-1"), expected);
}

#[test]
fn public_base_url_overrides_the_bind_address() {
    assert_eq!(
        run_url(Some("https://moadim.example.ts.net/"), "r1", "run-1"),
        "https://moadim.example.ts.net/runs/r1/run-1"
    );
}

#[test]
fn global_public_base_url_reads_table_and_flat_forms() {
    let home = temp_dir("base-url-home");
    let _home = EnvGuard::set("MOADIM_HOME_OVERRIDE", &home);
    assert_eq!(global_public_base_url(), None);
    write_global(
        "[notifications]\non_failure_command = \"echo hi\"\npublic_base_url = \"https://a.example\"\n",
    );
    assert_eq!(
        global_public_base_url().as_deref(),
        Some("https://a.example")
    );
    assert_eq!(
        global_config().on_failure_command.as_deref(),
        Some("echo hi")
    );
    write_global("public_base_url = \"https://b.example\"\n");
    assert_eq!(
        global_public_base_url().as_deref(),
        Some("https://b.example")
    );
}

fn command_env(exit_code: Option<i32>, status: RunStatus, name: &str) -> String {
    let dir = temp_dir(name);
    let _home = EnvGuard::set("MOADIM_HOME_OVERRIDE", &dir);
    let _machine = EnvGuard::set("MOADIM_MACHINE", "host-a");
    let _addr = EnvGuard::set("MOADIM_BIND_ADDR", "127.0.0.1:6123");
    let _shell = EnvGuard::set("MOADIM_SH_BIN", "/bin/sh");
    let out = dir.join("hook.out");
    let command = format!(
        "printf '%s|%s|%s' \"$MOADIM_MACHINE\" \"${{MOADIM_EXIT_CODE-unset}}\" \"$MOADIM_RUN_URL\" > {}",
        out.display()
    );
    let routine = super::routine(FailureNotificationConfig {
        on_failure_command: Some(command),
        on_failure_webhook: None,
    });
    notify_finished_run(&routine, "run-9", &dir, 1, 2, status, exit_code);
    wait_for(&out, |text| !text.is_empty())
}

#[test]
fn command_hook_receives_machine_exit_code_and_run_url() {
    assert_eq!(
        command_env(Some(7), RunStatus::Failed, "ctx-exit"),
        "host-a|7|http://127.0.0.1:6123/runs/routine-1/run-9"
    );
}

#[test]
fn command_hook_leaves_exit_code_unset_without_a_numeric_exit() {
    assert_eq!(
        command_env(None, RunStatus::Unknown, "ctx-unknown"),
        "host-a|unset|http://127.0.0.1:6123/runs/routine-1/run-9"
    );
}

#[test]
fn webhook_payload_carries_machine_null_exit_code_and_run_url() {
    let dir = temp_dir("ctx-webhook");
    let _home = EnvGuard::set("MOADIM_HOME_OVERRIDE", &dir);
    write_global("[notifications]\npublic_base_url = \"https://moadim.example\"\n");
    let _machine = EnvGuard::set("MOADIM_MACHINE", "host-b");
    let shim = dir.join("curl.sh");
    std::fs::write(
        &shim,
        "#!/bin/sh\nfor arg do target=\"$arg\"; done\ncat > \"$target\"\n",
    )
    .unwrap();
    std::process::Command::new("chmod")
        .arg("+x")
        .arg(&shim)
        .status()
        .unwrap();
    let _curl = EnvGuard::set(CURL_BIN_ENV, &shim);
    let sink = dir.join("sink.json");
    let mut routine = super::routine(FailureNotificationConfig::default());
    routine.notifications.on_failure_webhook = Some(sink.to_string_lossy().into_owned());
    notify_finished_run(&routine, "run-5", &dir, 1, 2, RunStatus::Unknown, None);
    let body = wait_for(&sink, |text| {
        serde_json::from_str::<serde_json::Value>(text).is_ok()
    });
    let sent: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(sent["machine"], "host-b");
    assert!(sent["exit_code"].is_null());
    assert_eq!(
        sent["run_url"],
        "https://moadim.example/runs/routine-1/run-5"
    );
}
