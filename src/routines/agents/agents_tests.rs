#![allow(clippy::missing_docs_in_private_items)]

use super::*;

/// A unique temp directory base for agent registry tests.
fn unique_dir(tag: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("moadim-agents-{tag}-{}", uuid::Uuid::new_v4()))
}

#[test]
fn available_agents_in_falls_back_when_dir_has_no_toml() {
    // Covers the `names.is_empty()` → built-in defaults branch when the directory
    // is readable but contains no `.toml` stems.
    let dir = unique_dir("empty-readable");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("notes.txt"), "ignore me").unwrap();

    assert_eq!(
        available_agents_in(&dir),
        vec![
            "claude".to_string(),
            "codex".to_string(),
            "hermes".to_string()
        ]
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn ensure_default_agents_seeds_into_override_home() {
    // Covers the public `ensure_default_agents` wrapper, which resolves `agents_dir()` through the
    // `MOADIM_HOME_OVERRIDE` seam and seeds the built-in configs there.
    let home = unique_dir("ensure-default");
    let previous = std::env::var_os("MOADIM_HOME_OVERRIDE");
    // SAFETY: tests run single-threaded (RUST_TEST_THREADS=1); the override is restored below.
    unsafe {
        std::env::set_var("MOADIM_HOME_OVERRIDE", &home);
    }

    ensure_default_agents();
    assert!(crate::paths::agents_dir().join("claude.toml").exists());

    // SAFETY: single-threaded harness; restore the saved value.
    unsafe {
        match previous {
            Some(value) => std::env::set_var("MOADIM_HOME_OVERRIDE", value),
            None => std::env::remove_var("MOADIM_HOME_OVERRIDE"),
        }
    }
    let _ = std::fs::remove_dir_all(&home);
}

#[test]
fn ensure_default_agents_in_returns_early_when_dir_is_uncreatable() {
    // Covers the `create_dir_all` error arm: a path whose parent is a regular file can never be
    // created, so the function logs and returns without writing any config.
    let base = unique_dir("uncreatable");
    std::fs::create_dir_all(&base).unwrap();
    let file = base.join("iamafile");
    std::fs::write(&file, "x").unwrap();
    let unmakeable = file.join("sub"); // parent is a file -> create_dir_all errors

    ensure_default_agents_in(&unmakeable);
    assert!(!unmakeable.exists());

    let _ = std::fs::remove_dir_all(&base);
}

#[cfg(unix)]
#[test]
fn ensure_default_agents_in_swallows_per_config_write_errors() {
    use std::os::unix::fs::PermissionsExt as _;

    // Covers the per-config `std::fs::write` error arm: the directory exists (so `create_dir_all`
    // succeeds) but is read-only, so each config write fails and is logged rather than panicking.
    let dir = unique_dir("write-fail");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o555)).unwrap();

    ensure_default_agents_in(&dir);

    // Restore permissions so cleanup can proceed. (Root bypasses the read-only bit, in which case
    // the writes succeed; the call is exercised either way.)
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755)).unwrap();
    let _ = std::fs::remove_dir_all(&dir);
}
