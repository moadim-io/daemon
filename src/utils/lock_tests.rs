use super::LockRecover;
use std::sync::{Arc, Mutex};
use std::thread;

/// A panic while holding the guard poisons the mutex; `lock_recover` must still
/// hand back the inner data instead of panicking like `.lock().unwrap()` would.
#[test]
fn lock_recover_returns_guard_after_poisoning() {
    let store: Arc<Mutex<Vec<i32>>> = Arc::new(Mutex::new(vec![1, 2, 3]));

    let poisoner = {
        let store = Arc::clone(&store);
        thread::spawn(move || {
            let mut guard = store.lock().unwrap();
            guard.push(4);
            panic!("poison the lock while it is held");
        })
    };
    // Join the panicking thread so the mutex is observably poisoned.
    assert!(poisoner.join().is_err());
    assert!(store.is_poisoned());

    // A plain `.lock().unwrap()` here would panic; `lock_recover` must not.
    let guard = store.lock_recover();
    assert_eq!(&*guard, &[1, 2, 3, 4]);
}

/// On a healthy (un-poisoned) mutex `lock_recover` behaves like a normal lock.
#[test]
fn lock_recover_acts_as_plain_lock_when_healthy() {
    let store = Mutex::new(10);
    *store.lock_recover() += 5;
    assert_eq!(*store.lock_recover(), 15);
}
