use std::process::{Child, Command};
use std::thread::{self, JoinHandle};

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
