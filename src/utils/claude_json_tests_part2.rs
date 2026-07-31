
#[test]
fn lock_exclusive_and_unlock_round_trip_on_a_real_file() {
    let dir = temp_dir("flock-roundtrip");
    let path = dir.join("lockfile");
    let file = File::create(&path).unwrap();

    lock_exclusive(&file).unwrap();
    unlock(&file).unwrap();

    fs::remove_dir_all(&dir).unwrap();
}

#[cfg(unix)]
#[test]
fn prune_project_at_errors_when_lock_file_cannot_be_created() {
    use std::os::unix::fs::PermissionsExt as _;

    // No `.claude.json.lock` exists yet, and the containing directory is read-only, so
    // `File::create(&lock_path)` in `prune_project_at` fails with a permission error —
    // exercising that `?` without touching `prune_locked` at all.
    let dir = temp_dir("lock-create-denied");
    let claude_json = dir.join(".claude.json");
    fs::write(&claude_json, r#"{"projects":{}}"#).unwrap();
    fs::set_permissions(&dir, fs::Permissions::from_mode(0o555)).unwrap();

    let result = prune_project_at(Some(claude_json), Path::new("/workbenches/gone"));

    fs::set_permissions(&dir, fs::Permissions::from_mode(0o755)).unwrap();
    assert!(result.is_err());
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn prune_project_at_errors_when_claude_json_is_a_directory() {
    // `claude_json.exists()` is true for a directory too, so this reaches `prune_locked`, where
    // `fs::read_to_string` fails because the path isn't a regular file.
    let dir = temp_dir("claude-json-is-dir");
    let claude_json = dir.join(".claude.json");
    fs::create_dir(&claude_json).unwrap();

    let result = prune_project_at(Some(claude_json), Path::new("/workbenches/gone"));

    assert!(result.is_err());
    fs::remove_dir_all(&dir).unwrap();
}

#[cfg(unix)]
#[test]
fn prune_project_at_errors_when_atomic_write_cannot_create_its_temp_file() {
    use std::os::unix::fs::PermissionsExt as _;

    // The lock file already exists, so `File::create` on it only needs to truncate an existing
    // directory entry (no directory-write permission required). The matching `projects` entry
    // still gets removed in memory, but the read-only directory then rejects the sibling temp
    // file `atomic_write` creates before its rename, exercising that `?` specifically.
    let dir = temp_dir("atomic-write-denied");
    let claude_json = dir.join(".claude.json");
    let workbench = "/home/u/.moadim/workbenches/my-routine-1700000000";
    fs::write(
        &claude_json,
        format!(r#"{{"projects":{{"{workbench}":{{}}}}}}"#),
    )
    .unwrap();
    fs::write(lock_path_for(&claude_json), "").unwrap();
    fs::set_permissions(&dir, fs::Permissions::from_mode(0o555)).unwrap();

    let result = prune_project_at(Some(claude_json), Path::new(workbench));

    fs::set_permissions(&dir, fs::Permissions::from_mode(0o755)).unwrap();
    assert!(result.is_err());
    fs::remove_dir_all(&dir).unwrap();
}

#[cfg(unix)]
#[test]
fn lock_exclusive_and_unlock_error_on_a_closed_fd() {
    use std::os::fd::AsRawFd as _;

    // `flock(2)` on a closed descriptor fails with EBADF, exercising the `Err` arm of both
    // `lock_exclusive` and `unlock` without depending on any real filesystem lock contention.
    let dir = temp_dir("flock-closed-fd");
    let path = dir.join("lockfile");
    let file = File::create(&path).unwrap();
    // SAFETY: `file` is not used again except via the (now-invalid) fd captured by `lock_exclusive`
    // and `unlock` below; this deliberately makes `flock` observe a closed descriptor.
    unsafe {
        libc::close(file.as_raw_fd());
    }

    assert!(lock_exclusive(&file).is_err());
    assert!(unlock(&file).is_err());

    #[allow(
        clippy::mem_forget,
        reason = "the fd was already manually closed above; letting `file`'s Drop run would close \
                  it a second time (or, worse, a reused fd from an unrelated file), so forgetting \
                  it is deliberate here, not a leak"
    )]
    std::mem::forget(file);
    fs::remove_dir_all(&dir).unwrap();
}
