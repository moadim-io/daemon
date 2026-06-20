//! Poison-resistant locking for the process-wide in-memory stores.
//!
//! The shared `CronStore` / `RoutineStore` are `Arc<Mutex<..>>` singletons. A
//! [`std::sync::Mutex`] becomes *poisoned* the instant any thread panics while
//! holding its guard, after which every later `.lock().unwrap()` panics too.
//! Because the stores are process-wide, a single stray panic under any held lock
//! would otherwise permanently brick the daemon — list/get/create/update/delete
//! /trigger, the iCal feed, crontab sync and the cleanup sweep all panic on their
//! next acquisition (#363).
//!
//! Recovering the guard via [`PoisonError::into_inner`] keeps the daemon serving:
//! the data behind the lock is still structurally valid (a `HashMap`), so the worst
//! case after a poisoning panic is a partially-applied mutation, not a dead daemon.

use std::sync::{Mutex, MutexGuard};

/// Acquire a [`Mutex`] guard, recovering the inner guard if the lock was poisoned.
pub trait LockRecover<T> {
    /// Lock the mutex, returning the guard even when the lock is poisoned.
    ///
    /// Equivalent to `self.lock().unwrap_or_else(|e| e.into_inner())`, but named so
    /// the poison-recovery intent is explicit at every call site and a plain
    /// `.lock().unwrap()` stands out in review.
    fn lock_recover(&self) -> MutexGuard<'_, T>;
}

impl<T> LockRecover<T> for Mutex<T> {
    fn lock_recover(&self) -> MutexGuard<'_, T> {
        self.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

#[cfg(test)]
#[path = "lock_tests.rs"]
mod lock_tests;
