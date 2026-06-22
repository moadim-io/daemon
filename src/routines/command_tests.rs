#![allow(clippy::missing_docs_in_private_items)]

use super::*;
use crate::routines::model::Routine;

/// Build a minimal routine for command-construction tests.
fn make_routine(title: &str) -> Routine {
    Routine {
        id: "cmd-test-id".to_string(),
        schedule: "@daily".to_string(),
        title: title.to_string(),
        agent: "claude".to_string(),
        prompt: "do it".to_string(),
        repositories: vec![],
        machines: vec![crate::machine::current_machine()],
        enabled: true,
        source: "managed".to_string(),
        created_at: 0,
        updated_at: 0,
        last_manual_trigger_at: None,
        last_scheduled_trigger_at: None,
        ttl_secs: None,
        max_runtime_secs: None,
    }
}

/// Run `body` with `PATH` set to `value`, restoring the previous value afterwards.
///
/// The test harness is single-threaded (`RUST_TEST_THREADS=1`), so mutating the
/// process-global `PATH` and restoring it around the call is safe.
fn with_path(value: &std::path::Path, body: impl FnOnce()) {
    let saved = std::env::var_os("PATH");
    // SAFETY: single-threaded test harness; the value is restored immediately after.
    unsafe {
        std::env::set_var("PATH", value);
    }
    body();
    unsafe {
        match saved {
            Some(prev) => std::env::set_var("PATH", prev),
            None => std::env::remove_var("PATH"),
        }
    }
}

#[test]
fn build_routine_command_resolves_bin_dir_when_tool_on_path() {
    // Place a fake `tmux` executable in a temp dir and point PATH at it, so
    // `cron_path` -> `bin_dir("tmux")` actually *finds* the binary. This exercises the
    // `.find(..).map(str::to_string)` Some-resolution in `bin_dir` and the
    // `if let Some(dir) { dirs.push(dir) }` arm in `cron_path` — the path taken only when
    // a tool is present on PATH.
    let dir = std::env::temp_dir().join(format!("moadim-cmd-path-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let tmux = dir.join("tmux");
    std::fs::write(&tmux, "#!/bin/sh\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(&tmux, std::fs::Permissions::from_mode(0o755)).unwrap();
    }

    let dir_str = dir.to_string_lossy().into_owned();
    with_path(&dir, || {
        let routine = make_routine("Cmd Path Routine");
        let agent = AgentCommand {
            command: "claude".to_string(),
            args: vec![],
            setup: None,
        };
        let cmd = build_routine_command(&routine, &agent);
        // The resolved tmux dir is baked into the exported PATH.
        assert!(
            cmd.contains(&dir_str),
            "expected resolved tmux dir {dir_str} in: {cmd}"
        );
    });

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn build_routine_command_stamps_scheduled_trigger_sidecar() {
    // The generated launch script records each scheduled firing by writing `$TS` into the
    // routine's `scheduled.local.toml` sidecar (best-effort, before the prompt-copy guard), since
    // the OS crontab runs this script directly without the daemon observing the fire.
    let routine = make_routine("Cmd Scheduled Stamp Routine");
    let agent = AgentCommand {
        command: "claude".to_string(),
        args: vec![],
        setup: None,
    };
    let cmd = build_routine_command(&routine, &agent);
    let sidecar = crate::paths::routine_scheduled_state_path(&slugify(&routine.title))
        .to_string_lossy()
        .into_owned();
    assert!(
        cmd.contains(&format!(
            r#"printf 'last_scheduled_trigger_at = %s\n' "$TS" > {} || true"#,
            shell_quote(&sidecar)
        )),
        "expected scheduled-trigger sidecar stamp in: {cmd}"
    );
    // It must run before the prompt-copy guard so an aborted run still records the firing.
    let stamp = cmd.find("last_scheduled_trigger_at").unwrap();
    let copy = cmd.find("/prompt.md\"").unwrap();
    assert!(stamp < copy, "sidecar stamp must precede the prompt copy");
}

#[test]
fn build_routine_command_uses_collision_resistant_run_id() {
    // Two runs of the same routine within the same wall-clock second must not collide on the
    // workbench dir or tmux session name. The run id mixes the launching shell's PID (`$$`, distinct
    // across concurrently-live processes) into `$TS`, and `$WB`/`$SESS` derive from that id. (#411)
    let routine = make_routine("Cmd Run Id Routine");
    let agent = AgentCommand {
        command: "claude".to_string(),
        args: vec![],
        setup: None,
    };
    let cmd = build_routine_command(&routine, &agent);

    // The run id carries the PID for intra-second uniqueness (joined with `_` so the slug and
    // timestamp stay recoverable by `parse_workbench_name`).
    assert!(
        cmd.contains(r#"RID="${TS}_$$""#),
        "expected PID-mixed run id in: {cmd}"
    );
    // Workbench and session derive from the run id, not the bare second-granularity `$TS`.
    assert!(
        cmd.contains(r#"WB="$HOME/.moadim/workbenches/$SLUG-$RID""#),
        "workbench must use the run id: {cmd}"
    );
    assert!(
        cmd.contains(r#"SESS="moadim-$SLUG-$RID""#),
        "session name must use the run id: {cmd}"
    );
    assert!(
        !cmd.contains("$SLUG-$TS"),
        "workbench/session must not fall back to second-granularity `$TS`: {cmd}"
    );
}

#[test]
fn build_routine_command_fails_loudly_on_session_collision() {
    // If `tmux new-session` still fails (e.g. a residual session collision), the launch must abort
    // non-zero rather than silently no-op — mirroring the prompt-copy guard. (#411)
    let routine = make_routine("Cmd Session Guard Routine");
    let agent = AgentCommand {
        command: "claude".to_string(),
        args: vec![],
        setup: None,
    };
    let cmd = build_routine_command(&routine, &agent);
    assert!(
        cmd.contains(r#"tmux new-session -d -s "$SESS" -c "$WB""#)
            && cmd.contains(r#"aborting launch" | tee -a "$WB/agent.log" >&2; exit 1; }"#),
        "tmux new-session must abort loudly on failure: {cmd}"
    );
}

#[test]
fn cron_path_falls_back_to_root_home_when_home_unset() {
    // With HOME removed, `std::env::var("HOME").unwrap_or_else(|_| "/root".to_string())` takes its
    // fallback arm, so the `~/.local/bin` etc. entries are anchored under `/root`.
    let saved = std::env::var_os("HOME");
    // SAFETY: single-threaded test harness; restored immediately below.
    unsafe {
        std::env::remove_var("HOME");
    }

    let path = cron_path("definitely-not-a-real-binary-xyz");
    assert!(
        path.contains("/root/.local/bin"),
        "expected /root-anchored fallback dirs in: {path}"
    );

    unsafe {
        match saved {
            Some(prev) => std::env::set_var("HOME", prev),
            None => std::env::remove_var("HOME"),
        }
    }
}

#[test]
fn bin_dir_returns_none_when_path_unset() {
    // With PATH removed entirely, `std::env::var("PATH").ok()?` short-circuits to None.
    let saved = std::env::var_os("PATH");
    // SAFETY: single-threaded test harness; restored immediately below.
    unsafe {
        std::env::remove_var("PATH");
    }
    assert!(bin_dir("definitely-not-a-real-binary-xyz").is_none());
    unsafe {
        if let Some(prev) = saved {
            std::env::set_var("PATH", prev);
        }
    }
}
