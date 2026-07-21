//! Resolution of `tmux` and agent-command binaries on `PATH` (and cron's minimal `PATH`), split
//! out of `command.rs` to keep that file under the line-count gate.

/// Return the first directory on the daemon's `PATH` that contains an executable named `bin`.
pub(crate) fn bin_dir(bin: &str) -> Option<String> {
    let path = std::env::var("PATH").ok()?;
    bin_dir_in(&path, bin)
}

/// Return the first directory in the `:`-separated `path` list that contains a file named `bin`.
///
/// Split out from [`bin_dir`] so the resolution logic is injectable in tests: callers can point
/// `path` at a temp dir with or without a fake binary without mutating the process-global `PATH`.
pub(crate) fn bin_dir_in(path: &str, bin: &str) -> Option<String> {
    path.split(':')
        .filter(|dir| !dir.is_empty())
        .find(|dir| std::path::Path::new(dir).join(bin).is_file())
        .map(str::to_string)
}

/// Whether `tmux` resolves to a file on the given `:`-separated `path` list.
///
/// `tmux` is a hard runtime dependency: routine launches run `tmux new-session … \; pipe-pane …`
/// and a missing `tmux` would be silently ignored (the statement is one of several `;`-joined
/// steps), making the run a no-op. This helper surfaces its presence so startup can warn and
/// `GET /health` can report it.
/// Injectable for tests via the `path` argument; see [`tmux_available`] for the live-`PATH` variant.
pub(crate) fn tmux_available_in(path: &str) -> bool {
    bin_dir_in(path, "tmux").is_some()
}

/// Whether `tmux` resolves on the daemon's live `PATH`. Returns `false` when `PATH` is unset.
pub(crate) fn tmux_available() -> bool {
    std::env::var("PATH")
        .ok()
        .is_some_and(|path| tmux_available_in(&path))
}

/// Whether `command` resolves to a file on the given `:`-separated `path` list.
///
/// Generalizes [`tmux_available_in`] to an arbitrary executable name: a routine's agent `command`
/// (e.g. `claude`, `codex`) is launched the same way `tmux` is — unresolved, it makes the cron
/// firing a silent no-op. Used to distinguish "agent config present" from "agent binary actually
/// runnable" in [`crate::routines::model::RoutineResponse`]. Injectable for tests via the `path` argument;
/// see [`agent_command_available`] for the live-`PATH` variant.
pub(crate) fn agent_command_available_in(path: &str, command: &str) -> bool {
    bin_dir_in(path, command).is_some()
}

/// Whether `command` resolves on the daemon's live `PATH`. Returns `false` when `PATH` is unset.
pub(crate) fn agent_command_available(command: &str) -> bool {
    std::env::var("PATH")
        .ok()
        .is_some_and(|path| agent_command_available_in(&path, command))
}

/// The first whitespace-delimited token of an agent's `setup` step — the interpreter or binary it
/// shells out to (e.g. `python3` for the built-in `claude` agent's workspace-trust seeding). `None`
/// for an empty/all-whitespace `setup` string.
///
/// `setup` is inserted verbatim into the launch command (see
/// [`super::command::build_routine_command`]), so this is a best-effort probe, not a shell parse:
/// it only catches the common "the step shells out to an interpreter that isn't installed" case
/// (issue #404), not every way a `setup` step can fail.
pub(crate) fn setup_step_interpreter(setup: &str) -> Option<&str> {
    setup.split_whitespace().next()
}

/// Whether an agent's `setup` step (if any) is safe to run: either there is no `setup` step, or
/// its [`setup_step_interpreter`] resolves on the daemon's live `PATH`. Mirrors
/// [`agent_command_available`] so a routine whose `setup` step would fail before the agent ever
/// launches (e.g. the built-in `claude` agent's `setup` shelling out to a missing `python3`) is
/// distinguishable from one that would actually run — see [`crate::routines::model::RoutineResponse`].
pub(crate) fn setup_step_available(setup: Option<&str>) -> bool {
    match setup.and_then(setup_step_interpreter) {
        Some(bin) => agent_command_available(bin),
        None => true,
    }
}

/// Common install locations to probe for `tmux` when it is not on `path` at all.
///
/// Split out so [`resolve_tmux_bin_from`] can be exercised in tests against fake, temp-dir-anchored
/// fallback lists instead of these real absolute paths (which may or may not hold a real `tmux` on
/// the machine running the tests).
pub(crate) fn tmux_fallback_dirs(home: &str) -> Vec<String> {
    vec![
        "/opt/homebrew/bin".to_string(),
        "/usr/local/bin".to_string(),
        format!("{home}/.local/bin"),
    ]
}

/// Best-effort absolute path to `tmux`: first dir on `path` holding it, else the first of
/// `fallback_dirs` holding it, else the bare `"tmux"` name.
///
/// Injectable variant of [`resolve_tmux_bin`] for tests. Mirrors the fallback list [`cron_path`]
/// bakes into crontab lines: launchd/systemd start the daemon with a minimal `PATH`
/// (`/usr/bin:/bin:/usr/sbin:/sbin`) that hides a Homebrew- or npm-installed `tmux`, so the
/// daemon's own tmux probes (`routines::cleanup::session`) would otherwise always fail to find it
/// — every liveness check then reads as "not running", so a hung run's workbench gets TTL-reaped
/// while the real tmux session and agent process are never killed and become permanently
/// untracked. Returning the bare `"tmux"` name when it cannot be found anywhere leaves the
/// caller's `Command::new` failing exactly as before.
pub(crate) fn resolve_tmux_bin_from(path: &str, fallback_dirs: &[String]) -> String {
    if let Some(dir) = bin_dir_in(path, "tmux") {
        return format!("{dir}/tmux");
    }
    for dir in fallback_dirs {
        if std::path::Path::new(dir).join("tmux").is_file() {
            return format!("{dir}/tmux");
        }
    }
    "tmux".to_string()
}

/// Live-`PATH`/`HOME` variant of [`resolve_tmux_bin_from`]; see its docs for why this exists.
pub(crate) fn resolve_tmux_bin() -> String {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
    let path = std::env::var("PATH").unwrap_or_default();
    resolve_tmux_bin_from(&path, &tmux_fallback_dirs(&home))
}

/// A short `PATH` for cron, since cron's default (`/usr/bin:/bin`) hides homebrew/npm-installed
/// tools like `tmux` and the agent binary.
///
/// Baking the daemon's full inherited `PATH` is not viable: it can exceed cron's per-line length
/// limit (~1000 chars) and silently disable the job. Instead this resolves just the dirs holding
/// `tmux` and the agent `command`, then appends common tool locations and the cron defaults,
/// deduplicated and order-preserving — short enough to stay well under the limit.
pub(crate) fn cron_path(agent_command: &str) -> String {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
    let mut dirs: Vec<String> = Vec::new();
    for bin in ["tmux", agent_command] {
        if let Some(dir) = bin_dir(bin) {
            dirs.push(dir);
        }
    }
    for dir in [
        format!("{home}/.local/bin"),
        "/opt/homebrew/bin".to_string(),
        "/usr/local/bin".to_string(),
        format!("{home}/.cargo/bin"),
        format!("{home}/.bun/bin"),
        "/usr/bin".to_string(),
        "/bin".to_string(),
        "/usr/sbin".to_string(),
        "/sbin".to_string(),
    ] {
        dirs.push(dir);
    }
    let mut seen = std::collections::HashSet::new();
    dirs.retain(|dir| seen.insert(dir.clone()));
    dirs.join(":")
}
