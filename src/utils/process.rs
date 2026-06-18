use std::process::Command;
use std::thread::JoinHandle;

/// Spawn `cmd` fire-and-forget, reaping the child on a detached thread.
///
/// Rust's stdlib does not `wait()` on a [`std::process::Child`] when it is dropped, so a child
/// spawned and immediately discarded becomes a `<defunct>` zombie that the kernel keeps around for
/// the daemon's entire lifetime. To avoid leaking zombies we hand the child to a detached thread
/// that blocks on [`std::process::Child::wait`], reaping it once it exits without holding up the
/// caller (the spawned tmux session / handler keeps running in the background).
///
/// Returns the reaper thread's [`JoinHandle`] on success so tests can join it and assert the child
/// was reaped; production callers simply ignore it.
pub fn spawn_reaped(cmd: &mut Command) -> std::io::Result<JoinHandle<()>> {
    let mut child = cmd.spawn()?;
    Ok(std::thread::spawn(move || {
        let _ = child.wait();
    }))
}

#[cfg(test)]
#[path = "process_tests.rs"]
mod process_tests;
