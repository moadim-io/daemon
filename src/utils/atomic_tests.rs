#![allow(clippy::missing_docs_in_private_items)]

use super::*;

/// Create and return a unique empty scratch directory under the system temp dir.
fn scratch_dir() -> PathBuf {
    let dir = std::env::temp_dir().join(format!("moadim-atomic-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Number of entries in `dir` whose name contains `.tmp`.
fn tmp_residue(dir: &Path) -> usize {
    std::fs::read_dir(dir)
        .unwrap()
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.file_name().to_string_lossy().contains(".tmp"))
        .count()
}

#[test]
fn writes_contents_and_leaves_no_tmp_residue() {
    let dir = scratch_dir();
    let target = dir.join("routine.toml");
    atomic_write(&target, b"hello").unwrap();

    assert_eq!(std::fs::read(&target).unwrap(), b"hello");
    assert_eq!(
        tmp_residue(&dir),
        0,
        "no .tmp file should remain on success"
    );

    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn overwrites_existing_file_atomically() {
    let dir = scratch_dir();
    let target = dir.join("prompt.md");
    atomic_write(&target, b"first").unwrap();
    atomic_write(&target, b"second").unwrap();

    assert_eq!(std::fs::read(&target).unwrap(), b"second");
    assert_eq!(tmp_residue(&dir), 0);

    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn errors_when_temp_cannot_be_created() {
    // Parent directory does not exist, so creating the sibling temp file fails.
    let target = std::env::temp_dir()
        .join(format!("moadim-atomic-missing-{}", Uuid::new_v4()))
        .join("routine.toml");
    let err = atomic_write(&target, b"data").unwrap_err();
    assert_eq!(err.kind(), io::ErrorKind::NotFound);
    assert!(!target.exists());
}

#[test]
fn errors_and_cleans_up_when_rename_fails() {
    // A directory already occupies the target path, so renaming the temp file over it fails. The
    // temp file must still be cleaned up, leaving no residue.
    let dir = scratch_dir();
    let target = dir.join("occupied");
    std::fs::create_dir(&target).unwrap();

    assert!(atomic_write(&target, b"data").is_err());
    assert_eq!(
        tmp_residue(&dir),
        0,
        "temp file should be removed on rename failure"
    );
    assert!(target.is_dir(), "the occupying directory is left untouched");

    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn tmp_path_falls_back_when_no_file_name() {
    // A path ending in `..` has no final component, exercising the `unwrap_or("tmp")` fallback.
    let tmp = tmp_path(Path::new("/some/dir/.."));
    assert!(tmp.to_string_lossy().contains(".tmp"));
}
