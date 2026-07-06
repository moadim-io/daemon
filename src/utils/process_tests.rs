use super::*;

/// A child that exits cleanly is spawned and reaped: `spawn_and_reap` returns a
/// join handle, and once that thread finishes the child has been `wait()`ed on
/// (no lingering zombie).
#[test]
fn spawn_and_reap_reaps_successful_child() {
    let mut command = Command::new("sh");
    command.arg("-c").arg("exit 0");
    let handle = spawn_and_reap(command, "test command").expect("spawn should succeed");
    // Joining the reaper guarantees `child.wait()` ran, deterministically reaping
    // the child before the test asserts.
    handle.join().expect("reaper thread should not panic");
}

/// A command that cannot be launched yields `None` and logs a warning rather
/// than panicking or leaking a thread.
#[test]
fn spawn_and_reap_returns_none_when_spawn_fails() {
    let command = Command::new("moadim-no-such-binary-zzz");
    assert!(spawn_and_reap(command, "missing binary").is_none());
}
