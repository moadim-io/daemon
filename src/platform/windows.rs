//! Windows backend: register moadim jobs/routines as Task Scheduler tasks (`schtasks`), launch
//! agents and handlers as detached processes, and check run liveness by PID.
//!
//! The Unix backend keeps its managed schedules inside delimited crontab blocks; on Windows each
//! managed entry becomes one scheduled task named `moadim-job-<id>` / `moadim-routine-<id>`. The set
//! of tasks under those prefixes is reconciled to match the store on every sync.

use std::collections::HashSet;
use std::os::windows::process::CommandExt as _;
use std::path::Path;
use std::process::Command;

use crate::sync::SyncError;

use super::schedule::cron_to_schtasks;

/// `CREATE_NO_WINDOW | DETACHED_PROCESS`: launch children without a console window and detached from
/// the daemon's console so a fire-and-forget trigger leaves no visible window behind.
const DETACHED_FLAGS: u32 = 0x0800_0000 | 0x0000_0008;

/// Task-name prefix for managed cron jobs.
pub const JOB_PREFIX: &str = "moadim-job-";
/// Task-name prefix for managed routines.
pub const ROUTINE_PREFIX: &str = "moadim-routine-";

/// A scheduled task to register: its unique task name, the cron schedule to translate, and the
/// command line passed to `schtasks /TR`.
pub struct SchedTask {
    /// Full task name, e.g. `moadim-routine-<uuid>`.
    pub name: String,
    /// The moadim/cron schedule (5-field or `@keyword`) to translate into Task Scheduler triggers.
    pub schedule: String,
    /// The command line `schtasks` runs when the trigger fires (the `/TR` value).
    pub run: String,
}

/// Build a `SyncError` for a failed `schtasks` invocation.
fn sched_err(msg: impl Into<String>) -> SyncError {
    SyncError::Scheduler(msg.into())
}

/// Return the names of existing scheduled tasks that start with `prefix`.
///
/// A `schtasks /Query` failure (e.g. no tasks yet) is treated as an empty list so a first-ever sync
/// still creates the desired tasks.
fn existing_task_names(prefix: &str) -> Vec<String> {
    let Ok(out) = Command::new("schtasks")
        .args(["/Query", "/FO", "CSV", "/NH"])
        .output()
    else {
        return Vec::new();
    };
    if !out.status.success() {
        return Vec::new();
    }
    let text = String::from_utf8_lossy(&out.stdout);
    text.lines()
        .filter_map(|line| line.split(',').next())
        .map(|field| field.trim().trim_matches('"').trim_start_matches('\\'))
        .filter(|name| name.starts_with(prefix))
        .map(str::to_string)
        .collect()
}

/// Delete the scheduled task named `name`. Best-effort: logged, never fatal.
fn delete_task(name: &str) {
    let status = Command::new("schtasks")
        .args(["/Delete", "/F", "/TN", name])
        .output();
    if let Err(err) = status {
        log::warn!("schtasks: failed to delete task {name:?}: {err}");
    }
}

/// Create (or overwrite, via `/F`) the scheduled task `task` with the given trigger flags.
fn create_task(task: &SchedTask, trigger: &[String]) -> Result<(), SyncError> {
    let mut args: Vec<&str> = vec!["/Create", "/F", "/TN", &task.name];
    args.extend(trigger.iter().map(String::as_str));
    args.push("/TR");
    args.push(&task.run);
    let out = Command::new("schtasks")
        .args(&args)
        .output()
        .map_err(|err| sched_err(format!("failed to run schtasks /Create: {err}")))?;
    if !out.status.success() {
        return Err(sched_err(format!(
            "schtasks /Create for {:?} failed: {}",
            task.name,
            String::from_utf8_lossy(&out.stderr).trim()
        )));
    }
    Ok(())
}

/// Reconcile the scheduled tasks under `prefix` to exactly match `desired`.
///
/// Tasks whose schedule has no Task Scheduler equivalent are skipped with a warning and treated as
/// not-present (so any stale task for that entry is removed rather than left firing on an old
/// trigger). Returns the first hard `schtasks` error, if any.
pub fn reconcile(prefix: &str, desired: &[SchedTask]) -> Result<(), SyncError> {
    let mut creatable: Vec<(&SchedTask, Vec<String>)> = Vec::new();
    for task in desired {
        match cron_to_schtasks(&task.schedule) {
            Some(trigger) => creatable.push((task, trigger)),
            None => log::warn!(
                "schtasks: schedule {:?} for task {:?} has no Task Scheduler equivalent; skipping",
                task.schedule,
                task.name
            ),
        }
    }

    let keep: HashSet<&str> = creatable
        .iter()
        .map(|(task, _)| task.name.as_str())
        .collect();
    for name in existing_task_names(prefix) {
        if !keep.contains(name.as_str()) {
            delete_task(&name);
        }
    }

    for (task, trigger) in &creatable {
        create_task(task, trigger)?;
    }
    Ok(())
}

/// The full path to `powershell.exe`, falling back to the bare name (resolved via `PATH`).
fn powershell() -> String {
    "powershell".to_string()
}

/// Build the `schtasks /TR` command line that runs a routine's `run.ps1` script.
pub fn routine_run_command(script: &Path) -> String {
    format!(
        "\"{}\" -NoProfile -ExecutionPolicy Bypass -File \"{}\"",
        powershell(),
        script.display()
    )
}

/// Spawn a detached PowerShell process running `script_body` (the `run.ps1` contents). Used by the
/// manual trigger so the API call returns immediately while the agent runs in the background.
pub fn spawn_routine_now(script_body: &str) {
    let spawned = Command::new(powershell())
        .args([
            "-NoProfile",
            "-ExecutionPolicy",
            "Bypass",
            "-Command",
            script_body,
        ])
        .creation_flags(DETACHED_FLAGS)
        .spawn();
    if let Err(err) = spawned {
        log::warn!("trigger: failed to spawn routine PowerShell: {err}");
    }
}

/// Spawn a cron handler at `path` as a detached process. `.ps1` handlers run via PowerShell; other
/// handlers (`.bat`/`.cmd`/`.exe`) run via `cmd /C` so batch files resolve.
pub fn spawn_handler(path: &Path) {
    let is_ps1 = path
        .extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("ps1"));
    let spawned = if is_ps1 {
        Command::new(powershell())
            .args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-File"])
            .arg(path)
            .creation_flags(DETACHED_FLAGS)
            .spawn()
    } else {
        Command::new("cmd")
            .arg("/C")
            .arg(path)
            .creation_flags(DETACHED_FLAGS)
            .spawn()
    };
    if let Err(err) = spawned {
        log::warn!("trigger: failed to spawn handler {path:?}: {err}");
    }
}

/// Whether the run for `session` (`moadim-{slug}-{ts}`) is still alive.
///
/// The Unix backend asks tmux; on Windows each `run.ps1` records its own PID in the workbench's
/// `agent.pid` while the agent runs and removes it on a clean exit. A missing or stale pid file
/// (process gone) reads as not-alive, so the cleanup task may reap a finished workbench. Mirrors the
/// Unix contract: when liveness can't be confirmed, the workbench is considered safe to reap.
pub fn run_alive(session: &str) -> bool {
    let name = session.strip_prefix("moadim-").unwrap_or(session);
    let pid_path = crate::paths::workbenches_dir().join(name).join("agent.pid");
    let Ok(text) = std::fs::read_to_string(&pid_path) else {
        return false;
    };
    let Ok(pid) = text.trim().parse::<u32>() else {
        return false;
    };
    pid_alive(pid)
}

/// Whether a process with `pid` is currently running, via `tasklist`.
fn pid_alive(pid: u32) -> bool {
    let Ok(out) = Command::new("tasklist")
        .args(["/FI", &format!("PID eq {pid}"), "/NH", "/FO", "CSV"])
        .output()
    else {
        return false;
    };
    if !out.status.success() {
        return false;
    }
    // When no task matches, tasklist prints an informational line without the PID; a match lists the
    // process row, which contains the PID as its own quoted CSV field.
    let text = String::from_utf8_lossy(&out.stdout);
    text.contains(&format!("\"{pid}\""))
}
