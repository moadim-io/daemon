//! Detached-process lifecycle, pid file, config-dir seeding, and the minimal loopback HTTP client.

use std::io::{Read as _, Write as _};
use std::net::SocketAddr;
use std::time::{Duration, SystemTime};

#[allow(
    clippy::missing_docs_in_private_items,
    reason = "split-out module keeps the file under the linecheck limit"
)]
#[path = "system_part2.rs"]
mod system_part2;
pub(crate) use system_part2::*;

/// How long to wait when probing or signalling a running server over HTTP.
const PROBE_TIMEOUT: Duration = Duration::from_millis(750);

/// Size at which `daemon.log` is rotated to `daemon.log.1` on the next detached spawn. The daemon
/// is long-lived and appends to this file on every start/restart with no other trim point, so
/// without a cap it grows unbounded until it fills the disk (#316).
pub(crate) const DAEMON_LOG_MAX_BYTES: u64 = 10 * 1024 * 1024;

/// How long a `daemon.log` segment may age before it is rotated regardless of size. The size cap
/// above only trims the file at the next detached spawn; a daemon that runs for weeks without a
/// restart and stays under [`DAEMON_LOG_MAX_BYTES`] would otherwise never rotate at all (#1157).
pub(crate) const DAEMON_LOG_MAX_AGE: Duration = Duration::from_secs(24 * 60 * 60);

/// How often the running daemon re-checks whether `daemon.log` is due for rotation. Cheap (a
/// single `stat`), so an interval well under [`DAEMON_LOG_MAX_AGE`] keeps the daily trigger from
/// drifting far past its target without needing a dedicated timer thread (#1157).
pub(crate) const LOG_ROTATION_CHECK_INTERVAL: Duration = Duration::from_secs(60 * 60);

/// Whether a `daemon.log` of `size` bytes, created `age` ago, is due for rotation. Split out from
/// [`rotate_daemon_log_if_due`] so the trigger logic is testable with injected size/age values
/// instead of needing to fake a real file's birth time.
fn log_rotation_is_due(size: u64, age: Duration) -> bool {
    size > DAEMON_LOG_MAX_BYTES || age > DAEMON_LOG_MAX_AGE
}

/// Rotate `log_path` to a sibling `.1` file (overwriting any previous one) if it is due per
/// [`log_rotation_is_due`] — oversized, or older than [`DAEMON_LOG_MAX_AGE`] since creation.
/// Called both at detached-spawn time and, by the running server's periodic tick, on
/// [`LOG_ROTATION_CHECK_INTERVAL`] — so a long-lived daemon rotates without needing a restart.
/// Best-effort: a failed rotation (permissions, race) or a filesystem that can't report a birth
/// time (age treated as zero, so only the size check applies) falls through to the caller rather
/// than blocking.
pub(crate) fn rotate_daemon_log_if_due(log_path: &std::path::Path) {
    let Ok(metadata) = std::fs::metadata(log_path) else {
        return;
    };
    let age = metadata
        .created()
        .ok()
        .and_then(|created| SystemTime::now().duration_since(created).ok())
        .unwrap_or_default();
    if !log_rotation_is_due(metadata.len(), age) {
        return;
    }
    let rotated_path = log_path.with_extension("log.1");
    let _ = std::fs::rename(log_path, rotated_path);
}

/// Write the current process PID into the pid file so `stop`/`status` and signals can find it.
pub fn write_pid_file() -> anyhow::Result<()> {
    let path = crate::paths::pid_file();
    let parent = crate::utils::fs_perms::parent_or_err(&path, "pid file")?;
    crate::utils::fs_perms::create_private_dir_all(parent)?;
    ensure_config_gitignore();
    ensure_readme(&crate::paths::config_readme_path(), CONFIG_README);
    ensure_readme(&crate::paths::routines_readme_path(), ROUTINES_README);
    ensure_readme(&crate::paths::agents_readme_path(), AGENTS_README);
    std::fs::write(&path, std::process::id().to_string())?;
    Ok(())
}

/// Orientation doc seeded into the config dir on every start; see [`ensure_readme`].
const CONFIG_README: &str = "\
# moadim config

This is moadim's config directory (`$XDG_CONFIG_HOME/moadim`, default `~/.config/moadim`).
It is git-trackable — commit it (or the parts you want to keep) to version-control your
routines and agents across machines.

- `routines/` — one directory per routine (a scheduled agent); see its own `README.md`.
- `agents/` — the agent registry referenced by routines; see its own `README.md`.
- `machine.local.toml` — this machine's identity, used to match a routine's `machines`
  targeting list. Gitignored: it's per-machine, not shared.
- `moadim.pid`, `daemon.log` — daemon-managed runtime files. Gitignored.
- `.gitignore` — seeded and kept up to date by the daemon; append your own patterns freely.

Full docs: https://github.com/moadim-io/daemon
";

/// Orientation doc seeded into `routines/` on every start; see [`ensure_readme`].
const ROUTINES_README: &str = "\
# moadim routines

Each subdirectory here is one routine (a prompt + schedule + agent, run on a cron schedule).

- `<id>/routine.toml` — the agent, repositories, machine targeting, and metadata.
- `<id>/schedule.cron` — the human-authored cron schedule(s).
- `<id>/schedule.compailed.cron` — gitignored cron-union output used for OS crontab sync.
- `<id>/prompts/prompt.pure.md` — the prompt you wrote.
- `<id>/prompts/prompt.compiled.local.md` — the composed prompt (repositories preamble +
  pure prompt) copied into each run's workbench. Gitignored (`.local.` matches the
  `*.local.*` pattern): it's fully derived from `prompt.pure.md` + `routine.toml` and
  rewritten on every save.
- `<id>/flags/` — open questions an agent raised mid-run: a gap, bug, edge case, or question
  it couldn't resolve.
- `<id>/state.local.toml` — gitignored sidecar holding snooze/skip-runs state.
- `<id>/manual.log`, `<id>/scheduled.log` — gitignored append-only logs recording every
  manual / scheduled trigger (one Unix timestamp per line).

Full docs: https://github.com/moadim-io/daemon
";

/// Orientation doc seeded into `agents/` on every start; see [`ensure_readme`].
const AGENTS_README: &str = "\
# moadim agents

The agent registry referenced by routines. Each `<name>.toml` here (e.g. `claude.toml`)
describes one coding agent: the command to launch it and any agent-specific settings.
Routines reference an agent by name in their `routine.toml`.

Full docs: https://github.com/moadim-io/daemon
";

/// Seed `path` with `content` if it doesn't already exist, creating its parent directory as
/// needed.
///
/// Runs on every start for the config dir and each of its generated subdirectories
/// (`routines/`, `agents/`), alongside [`ensure_config_gitignore`]. Only writes when the file is
/// missing, so a user's edits are never clobbered. Best-effort: failure is not fatal.
fn ensure_readme(path: &std::path::Path, content: &str) {
    if path.exists() {
        return;
    }
    let parent = crate::utils::fs_perms::parent_or_err(path, "readme");
    let Some(parent) = parent.ok() else { return };
    if crate::utils::fs_perms::create_private_dir_all(parent).is_err() {
        return;
    }
    let _ = std::fs::write(path, content);
}

/// Ensure the config dir `.gitignore` contains all required patterns on every start.
///
/// This is the single `.gitignore` for the whole config tree — its patterns apply recursively, so
/// they also cover every routine directory (per-routine `.gitignore` files are no longer
/// generated): `*.local.*` catches the machine-local sidecars (`state.local.toml`,
/// `routine.local.toml`, `prompt.compiled.local.md`), `*.log` the trigger logs, `*.compiled.*`
/// the legacy `prompts/prompt.compiled.md`, `schedule.compailed.cron` the cron-union output,
/// `.compailed.cron` the legacy cron-union output name, and `run.sh` the obsolete per-routine
/// launch script.
///
/// Reads the existing file (if any), appends any missing patterns, and writes back only when
/// something changed. Preserves user-added entries. Best-effort: failure is not fatal.
fn ensure_config_gitignore() {
    const REQUIRED: &[&str] = &[
        "*.pid",
        "*.log",
        "*.local.*",
        "*.compiled.*",
        "schedule.compailed.cron",
        ".compailed.cron",
        "run.sh",
    ];
    let gitignore = crate::paths::config_gitignore_path();
    let existing = std::fs::read_to_string(&gitignore).unwrap_or_default();
    let lines: Vec<&str> = existing.lines().collect();
    let missing: Vec<&str> = REQUIRED
        .iter()
        .copied()
        .filter(|pat| !lines.iter().any(|line| line.trim() == *pat))
        .collect();
    if missing.is_empty() {
        return;
    }
    let mut content = existing;
    if !content.is_empty() && !content.ends_with('\n') {
        content.push('\n');
    }
    for pattern in &missing {
        content.push_str(pattern);
        content.push('\n');
    }
    let _ = std::fs::write(&gitignore, &content);
}

/// Remove the pid file. Best-effort: a missing file is not an error.
pub fn clear_pid_file() {
    let _ = std::fs::remove_file(crate::paths::pid_file());
}

/// Read the PID recorded in the pid file, if present, parseable, **and still a live process**.
///
/// On a clean shutdown the pid file is removed ([`clear_pid_file`]), but a `kill -9`, panic, OOM
/// kill, or power loss skips that path and leaves the file behind with a now-dead PID. Returning
/// that stale PID would make the machine-readable contract dishonest — `status`/`stop --json` would
/// report a `pid` for a process that no longer exists (and which, after PID reuse, may belong to an
/// unrelated process), and `restart` would force-kill it. So a recorded PID that is not alive is
/// treated as absent and the stale file is cleaned up best-effort, keeping the pid self-healing.
pub(crate) fn read_pid_file() -> Option<u32> {
    let pid = std::fs::read_to_string(crate::paths::pid_file())
        .ok()?
        .trim()
        .parse()
        .ok()?;
    if process_is_alive(pid) {
        Some(pid)
    } else {
        clear_pid_file();
        None
    }
}

/// Returns `true` if a process with `pid` currently exists.
///
/// Uses the standard signal-0 liveness probe (`kill -0 <pid>`): no signal is delivered, but the
/// kernel runs the same existence check it would for a real signal, so a successful exit means the
/// PID is alive. Unlike [`crate::restart`]'s destructive killer, this probe is harmless, so it goes
/// straight to the real `kill` with no env override to keep parallel tests from racing on it.
#[cfg(unix)]
fn process_is_alive(pid: u32) -> bool {
    // Linux PIDs are signed; u32::MAX overflows to -1 in the kernel, causing kill(-1, 0)
    // to return success (it sends to all accessible processes). Treat out-of-range as dead.
    if pid > i32::MAX as u32 {
        return false;
    }
    std::process::Command::new("kill")
        .args(["-0", &pid.to_string()])
        .output()
        .is_ok_and(|out| out.status.success())
}

/// Best-effort liveness on non-unix: assume the recorded PID is alive, mirroring the best-effort
/// semantics of the Windows force-killer. The pid file is still cleared on graceful shutdown.
#[cfg(not(unix))]
fn process_is_alive(_pid: u32) -> bool {
    true
}

/// The daemon log path, rendered for display.
pub(super) fn paths_daemon_log() -> String {
    crate::paths::daemon_log_file().display().to_string()
}

/// Returns `true` if a server answers `GET /health` on [`super::BIND_ADDR`].
pub(crate) fn is_running() -> bool {
    matches!(http_request("GET", "/api/v1/health"), Ok(200))
}

/// Interval between liveness probes while [`wait_until`] polls for a server to come up.
const WAIT_POLL_INTERVAL: Duration = Duration::from_millis(200);

#[cfg(test)]
#[path = "system_tests.rs"]
mod cli_system_tests;
