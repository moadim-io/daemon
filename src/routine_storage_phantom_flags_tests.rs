use super::super::write_routine_to_rel_dir;
use super::{migrate_phantom_flags, move_phantom_flags, routines_dir};

fn temp_base(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("moadim-phantom-flags-{name}-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn moves_flags_from_a_phantom_slug_dir_and_removes_it() {
    let base = temp_base("move");
    std::fs::create_dir_all(base.join("my-routine/flags")).unwrap();
    std::fs::write(base.join("my-routine/flags/a.md"), "a").unwrap();
    std::fs::create_dir_all(base.join("folder/my-routine")).unwrap();

    move_phantom_flags(&base, "my-routine", "folder/my-routine");

    assert_eq!(std::fs::read_to_string(base.join("folder/my-routine/flags/a.md")).unwrap(), "a");
    assert!(!base.join("my-routine").exists());
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn keeps_a_colliding_flag_and_its_phantom_dir() {
    let base = temp_base("collide");
    std::fs::create_dir_all(base.join("old/flags")).unwrap();
    std::fs::write(base.join("old/flags/a.md"), "phantom").unwrap();
    std::fs::create_dir_all(base.join("new/flags")).unwrap();
    std::fs::write(base.join("new/flags/a.md"), "real").unwrap();

    move_phantom_flags(&base, "old", "new");

    assert_eq!(std::fs::read_to_string(base.join("new/flags/a.md")).unwrap(), "real");
    assert_eq!(std::fs::read_to_string(base.join("old/flags/a.md")).unwrap(), "phantom");
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn leaves_another_routines_real_dir_and_same_dir_alone() {
    let base = temp_base("real");
    std::fs::create_dir_all(base.join("taken/flags")).unwrap();
    std::fs::write(base.join("taken/routine.toml"), "").unwrap();
    std::fs::write(base.join("taken/flags/a.md"), "a").unwrap();

    move_phantom_flags(&base, "taken", "elsewhere");
    move_phantom_flags(&base, "taken", "taken");

    assert!(base.join("taken/flags/a.md").exists());
    assert!(!base.join("elsewhere").exists());
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn no_phantom_flags_dir_is_a_no_op() {
    let base = temp_base("none");
    move_phantom_flags(&base, "missing", "folder/missing");
    assert!(!base.join("folder").exists());
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn migrate_phantom_flags_moves_a_foldered_routines_slug_flags_into_its_real_dir() {
    let home = temp_base("home");
    let previous = std::env::var_os("MOADIM_HOME_OVERRIDE");
    // SAFETY: tests in this crate run single-threaded per binary; restored below.
    unsafe {
        std::env::set_var("MOADIM_HOME_OVERRIDE", &home);
    }
    let routine = crate::test_fixtures::routine_fixture("phantom-id", "My Routine")
        .no_repositories()
        .build();
    write_routine_to_rel_dir(&routine, "folder/my-routine").unwrap();
    let phantom = routines_dir().join("my-routine/flags");
    std::fs::create_dir_all(&phantom).unwrap();
    std::fs::write(phantom.join("a.md"), "a").unwrap();
    let store: crate::routines::RoutineStore = std::sync::Arc::new(std::sync::Mutex::new(
        std::collections::HashMap::from([(routine.id.clone(), routine)]),
    ));

    migrate_phantom_flags(&store);

    let moved = routines_dir().join("folder/my-routine/flags/a.md");
    assert_eq!(std::fs::read_to_string(moved).unwrap(), "a");
    assert!(!routines_dir().join("my-routine").exists());
    // SAFETY: single-threaded test execution.
    unsafe {
        match previous {
            Some(value) => std::env::set_var("MOADIM_HOME_OVERRIDE", value),
            None => std::env::remove_var("MOADIM_HOME_OVERRIDE"),
        }
    }
    let _ = std::fs::remove_dir_all(&home);
}

#[test]
fn keeps_phantom_flags_when_the_destination_cannot_be_created() {
    let base = temp_base("blocked");
    std::fs::create_dir_all(base.join("old/flags")).unwrap();
    std::fs::write(base.join("old/flags/a.md"), "a").unwrap();
    // A plain file where the destination routine dir should be makes `create_dir_all` fail.
    std::fs::write(base.join("new"), "not a dir").unwrap();

    move_phantom_flags(&base, "old", "new");

    assert!(base.join("old/flags/a.md").exists());
    let _ = std::fs::remove_dir_all(&base);
}
