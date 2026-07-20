use std::path::PathBuf;
use std::process::{Child, Command};
use std::thread::{self, JoinHandle};

/// Test-only env var: when set, [`current_exe`] returns an error instead of resolving the real
/// path. `std::env::current_exe()` failing is otherwise unreachable from a test — the OS syscall
/// only errors if the running binary's own file was deleted mid-execution or under unusual
/// sandboxing — so this seam exists purely to exercise that error branch in its callers, mirroring
/// the `MOADIM_CRONTAB_BIN`/`MOADIM_LAUNCHCTL_BIN` test seams for external-binary resolution.
#[cfg(test)]
pub const CURRENT_EXE_FAIL_ENV: &str = "MOADIM_CURRENT_EXE_FAIL_FOR_TEST";

/// Resolve the path to the currently running executable.
///
/// Wraps [`std::env::current_exe`]; see `CURRENT_EXE_FAIL_ENV` for the test-only failure seam.
pub fn current_exe() -> std::io::Result<PathBuf> {
    #[cfg(test)]
    if std::env::var_os(CURRENT_EXE_FAIL_ENV).is_some() {
        return Err(std::io::Error::other("forced current_exe failure for test"));
    }
    std::env::current_exe()
}

/// Spawn `command`, then hand the resulting child off to a detached reaper thread.
///
/// The daemon fires routine triggers from inside its own
/// long-running process. Rust's standard library does **not** reap a [`Child`]
/// when its handle is dropped, so a spawned process that exits would linger as a
/// zombie (`<defunct>`) in the process table for the daemon's entire lifetime,
/// slowly consuming PID-table slots. We [`wait`](Child::wait) on the child in a
/// background thread so the trigger stays fire-and-forget while the finished
/// child is still reaped.
///
/// `context` labels the spawn in the failure log. On success the reaper's
/// [`JoinHandle`] is returned so callers (and tests) may observe it; production
/// callers simply drop it and the thread runs to completion on its own.
pub fn spawn_and_reap(mut command: Command, context: &str) -> Option<JoinHandle<()>> {
    match command.spawn() {
        Ok(child) => Some(reap_in_background(child)),
        Err(err) => {
            log::warn!("trigger: failed to spawn {context}: {err}");
            None
        }
    }
}

/// Detach a thread that [`wait`](Child::wait)s on `child`, reaping it once it exits.
fn reap_in_background(mut child: Child) -> JoinHandle<()> {
    thread::spawn(move || {
        let _ = child.wait();
    })
}

#[cfg(test)]
#[path = "process_tests.rs"]
mod process_tests;
