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
        enabled: true,
        source: "managed".to_string(),
        created_at: 0,
        updated_at: 0,
        last_triggered_at: None,
        ttl_secs: None,
    }
}

/// Run `body` with `PATH` set to `value`, restoring the previous value afterwards.
///
/// The test harness is single-threaded (`RUST_TEST_THREADS=1`), so mutating the
/// process-global `PATH` and restoring it around the call is safe.
#[cfg(unix)]
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

#[cfg(unix)]
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

#[cfg(unix)]
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

#[cfg(unix)]
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

// ─── Windows (PowerShell) launch script ──────────────────────────────────────

#[cfg(windows)]
#[test]
fn build_routine_command_windows_emits_powershell() {
    let routine = make_routine("My Routine");
    let agent = AgentCommand {
        command: "claude".to_string(),
        args: vec!["--permission-mode".to_string(), "{prompt}".to_string()],
        setup: Some("seed-trust $wb".to_string()),
    };
    let cmd = build_routine_command(&routine, &agent);
    // Stamps a workbench, writes CLAUDE.md with the moadim prompt + disclosure, copies prompt.md.
    assert!(cmd.contains("New-Item -ItemType Directory"));
    assert!(cmd.contains("Moadim Context"));
    assert!(cmd.contains("Routine origin disclosure"));
    assert!(cmd.contains("'My Routine'"));
    assert!(cmd.contains("CLAUDE.md"));
    // Prompt passed as one argument via Get-Content -Raw (splatting).
    assert!(cmd.contains("Get-Content -Raw -LiteralPath (Join-Path $wb 'prompt.md')"));
    // Fail-fast on a missing source prompt, before the agent launches.
    let copy_at = cmd.find("Copy-Item").expect("copy present");
    let abort_at = cmd.find("aborting launch").expect("abort guard present");
    assert!(abort_at < copy_at, "abort guard must precede the copy");
    // setup runs verbatim before the agent launches.
    let setup_at = cmd.find("seed-trust $wb").expect("setup present");
    let launch_at = cmd.find("Tee-Object").expect("launch present");
    assert!(setup_at < launch_at);
    // records its PID so cleanup can tell a running session from a finished one.
    assert!(cmd.contains("$PID | Set-Content"));
}

#[cfg(windows)]
#[test]
fn build_routine_command_windows_substitutes_prompt_file() {
    let routine = make_routine("My Routine");
    let agent = AgentCommand {
        command: "codex".to_string(),
        args: vec!["exec".to_string(), "{prompt_file}".to_string()],
        setup: None,
    };
    let cmd = build_routine_command(&routine, &agent);
    assert!(cmd.contains("$agentArgs = @('exec', 'prompt.md')"));
    assert!(cmd.contains("& 'codex' @agentArgs"));
}
