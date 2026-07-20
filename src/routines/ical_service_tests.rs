//! Tests for the `svc_ical`/`svc_ical_routine`/`build_ical` service-layer entry points (store
//! reload, per-routine filtering, lock recovery) split out of `routines/ical_tests.rs` to stay
//! under the repo's per-file line gate.

use super::*;

#[test]
fn svc_ical_reads_store() {
    // `svc_ical` reloads from disk first: a routine present on disk is rendered even though the
    // in-memory store starts empty, proving the iCal read path re-scans the directory.
    let dir = scratch_dir();
    write_routine_to(&dir, &routine_with("r1", "@daily", true));
    let ics = svc_ical(&new_store(), &dir);
    assert!(ics.starts_with("BEGIN:VCALENDAR"));
    assert!(ics.contains("BEGIN:VEVENT"));
}

#[test]
fn svc_ical_routine_filters_to_one_routine() {
    // Two enabled routines in the store; the filtered feed contains only the requested
    // one's events, and the calendar is named after that routine (issue #263).
    let dir = scratch_dir();
    let mut keep = routine_with("keep", "@daily", true);
    keep.title = "Keep Me".to_string();
    write_routine_to(&dir, &keep);
    let mut other = routine_with("other", "@daily", true);
    other.title = "Other".to_string();
    write_routine_to(&dir, &other);
    let ics = svc_ical_routine(&new_store(), &dir, "keep");
    assert!(ics.contains("UID:keep-"));
    assert!(!ics.contains("UID:other-"));
    assert!(ics.contains("SUMMARY:Keep Me\r\n"));
    // Calendar is named after the routine, not the generic all-routines name.
    assert!(ics.contains("X-WR-CALNAME:Keep Me\r\n"));
    assert!(!ics.contains("X-WR-CALNAME:Moadim Routines\r\n"));
}

#[test]
fn svc_ical_routine_unknown_id_is_well_formed_empty_calendar() {
    // An unknown id is not an error: a valid, empty VCALENDAR with the default name.
    let dir = scratch_dir();
    write_routine_to(&dir, &routine_with("r1", "@daily", true));
    let ics = svc_ical_routine(&new_store(), &dir, "does-not-exist");
    assert!(ics.starts_with("BEGIN:VCALENDAR\r\n"));
    assert!(ics.contains("X-WR-CALNAME:Moadim Routines\r\n"));
    assert!(ics.ends_with("END:VCALENDAR\r\n"));
    assert_eq!(count(&ics, "BEGIN:VEVENT"), 0);
}

#[test]
fn svc_ical_routine_survives_a_poisoned_store_lock() {
    // A `std::sync::Mutex` poisons permanently the instant any thread panics while
    // holding the guard. `svc_ical_routine` reloads the store before serving (which itself
    // takes the lock via `LockRecover`) and must recover the guard — like every other store
    // accessor — instead of propagating that poisoning as its own panic — see
    // `utils::lock::LockRecover`.
    let dir = scratch_dir();
    write_routine_to(&dir, &routine_with("r1", "@daily", true));
    let store = new_store();

    let poisoner = std::sync::Arc::clone(&store);
    let handle = std::thread::spawn(move || {
        let _guard = poisoner.lock().expect("first lock is not yet poisoned");
        panic!("poison the routine store");
    });
    assert!(
        handle.join().is_err(),
        "the spawned thread should have panicked"
    );

    let ics = svc_ical_routine(&store, &dir, "r1");
    assert!(ics.starts_with("BEGIN:VCALENDAR\r\n"));
    assert!(ics.contains("BEGIN:VEVENT"));
}

#[test]
fn build_ical_skips_all_routines_when_globally_locked() {
    let dir = std::env::temp_dir().join(format!("moadim-icallock-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).expect("create temp home");
    // SAFETY: single-threaded test execution (RUST_TEST_THREADS=1).
    unsafe {
        std::env::set_var("MOADIM_HOME_OVERRIDE", &dir);
    }
    let lock_path = crate::paths::global_lock_path();
    if let Some(parent) = lock_path.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(&lock_path, b"").unwrap();

    let routine = routine_with("rl", "@daily", true);
    let ics = build_ical(&[routine], fixed_now());
    assert!(
        !ics.contains("BEGIN:VEVENT"),
        "globally locked feed must have no events"
    );

    // SAFETY: cleanup.
    unsafe {
        std::env::remove_var("MOADIM_HOME_OVERRIDE");
    }
    let _ = std::fs::remove_dir_all(&dir);
}
