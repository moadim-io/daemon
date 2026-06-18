#![allow(clippy::missing_docs_in_private_items)]

use super::*;

#[test]
fn spawn_reaped_reaps_a_fast_exiting_child() {
    // A child spawned and dropped without wait() would linger as a <defunct> zombie. The reaper
    // thread must wait() on it; joining the handle here proves the wait() ran to completion.
    let handle = spawn_reaped(&mut Command::new("true")).expect("spawn should succeed");
    handle.join().expect("reaper thread should not panic");
}

#[test]
fn spawn_reaped_returns_err_for_missing_binary() {
    // A non-existent program must surface the spawn error to the caller (which logs it) rather
    // than panicking or silently succeeding.
    let result = spawn_reaped(&mut Command::new("definitely-not-a-real-binary-zxcv-212"));
    assert!(result.is_err());
}
