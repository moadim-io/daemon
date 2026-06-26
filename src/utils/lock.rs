//! Poison-tolerant locking for the in-memory stores.
//!
//! Every store is an `Arc<Mutex<HashMap<..>>>` guarding plain data. A poisoned lock
//! only means some earlier thread panicked while holding the guard — the map itself is
//! still structurally valid — so recovering the guard keeps the daemon serving instead
//! of cascading the original panic through every later request.

use std::sync::{Mutex, MutexGuard};

/// Extension trait that locks a [`Mutex`] without panicking on poisoning.
pub trait LockRecover<Inner> {
    /// Lock the mutex, recovering the guard if a previous holder panicked while holding it.
    fn lock_recover(&self) -> MutexGuard<'_, Inner>;
}

impl<Inner> LockRecover<Inner> for Mutex<Inner> {
    fn lock_recover(&self) -> MutexGuard<'_, Inner> {
        self.lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

#[cfg(test)]
#[path = "lock_tests.rs"]
mod lock_tests;
