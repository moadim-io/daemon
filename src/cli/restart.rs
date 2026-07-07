//! The `restart` command and its detached-spawn reporting, split out of `cli/mod.rs` to stay under
//! the repo's per-file line gate.

use super::{bind_addr, paths_daemon_log, spawn_detached, stop_existing_for_restart};

/// Stop a running background server (if any) and start a fresh detached instance. With `json`,
/// emits a single machine-readable object (`{"old":N|null,"new":M}`) instead of the human-readable
/// lines.
///
/// Unlike [`super::run_background`], which restarts only as a side effect of being asked to start
/// while one is already up, this is the explicit "give me a clean process now" command: it stops
/// the running server when present, otherwise just starts one.
pub fn restart(json: bool, quiet: bool) -> anyhow::Result<()> {
    // Only the bare command narrates the stop/start step and prints the hint block; `--json` emits a
    // single object and `--quiet` prints just the rotation line.
    let old_pid = stop_existing_for_restart(json || quiet)?;
    let new_pid = spawn_detached()?;
    if json {
        println!("{}", restart_json(old_pid, new_pid));
    } else {
        // Headline the rotation so scripts/logs can see the process actually changed.
        println!("{}", restart_rotation_line(old_pid, new_pid));
        if !quiet {
            report_endpoints();
        }
    }
    Ok(())
}

/// Format the one-line PID rotation summary `restart` prints, e.g. `restarted: pid 123 -> 456`.
/// `old` reads `none` when nothing was running (or its PID could not be read).
pub(crate) fn restart_rotation_line(old: Option<u32>, new: u32) -> String {
    let old = old.map_or_else(|| "none".to_string(), |pid| pid.to_string());
    format!("restarted: pid {old} -> {new}")
}

/// Render the `restart` result as a one-line JSON object: `{"old":N|null,"new":N,"address":…}`.
/// `old` is `null` when nothing was running (mirroring [`restart_rotation_line`]'s `none`); `new`
/// is the freshly spawned PID; `address` is the bound [`super::BIND_ADDR`].
pub(crate) fn restart_json(old: Option<u32>, new: u32) -> String {
    serde_json::json!({
        "old": old,
        "new": new,
        "address": bind_addr(),
    })
    .to_string()
}

/// Spawn a detached server process and print where to reach and manage it.
///
/// `verb` describes how the process came to be ("started" / "restarted") for the first line.
pub(crate) fn start_detached_and_report(verb: &str) -> anyhow::Result<()> {
    let pid = spawn_detached()?;
    println!(
        "moadim {verb} in the background (pid {pid}) at http://{}",
        bind_addr()
    );
    report_endpoints();
    Ok(())
}

/// Print the reach/manage hints (UI, stop, logs) shared by every detached-launch report.
fn report_endpoints() {
    println!("  UI    http://{}", bind_addr());
    println!("  stop  moadim stop   (or use the STOP button in the UI)");
    println!("  logs  {}", paths_daemon_log());
}
