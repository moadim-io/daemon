//! Tests for [`super::LockRecover`].
#![allow(
    clippy::missing_docs_in_private_items,
    reason = "test helpers and fixtures do not need doc comments"
)]

use super::LockRecover;
use std::sync::{Arc, Mutex};

#[test]
fn lock_recover_returns_guard_for_healthy_mutex() {
    let mutex = Mutex::new(7_u32);
    assert_eq!(*mutex.lock_recover(), 7);
}

#[test]
fn lock_recover_recovers_a_poisoned_guard() {
    let mutex = Arc::new(Mutex::new(11_u32));
    let poisoner = Arc::clone(&mutex);
    let handle = std::thread::spawn(move || {
        let _guard = poisoner.lock().expect("first lock is not yet poisoned");
        panic!("poison the mutex");
    });
    assert!(
        handle.join().is_err(),
        "the spawned thread should have panicked"
    );

    // The mutex is now poisoned; lock_recover must hand back the inner value anyway.
    assert_eq!(*mutex.lock_recover(), 11);
}
