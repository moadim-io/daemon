#![allow(
    clippy::missing_docs_in_private_items,
    reason = "test helpers and cases do not need doc comments"
)]

use crate::routine_tags_csv::{
    format_routine_tags, read_routine_tags, resolve_tags, seed_tags_csv_from_legacy_toml,
};

fn scratch_dir(tag: &str) -> std::path::PathBuf {
    let dir =
        std::env::temp_dir().join(format!("moadim-rs-tags-csv-{tag}-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn strings(values: &[&str]) -> Vec<String> {
    values.iter().map(ToString::to_string).collect()
}

#[test]
fn read_returns_none_when_file_is_absent() {
    let dir = scratch_dir("absent");
    assert_eq!(read_routine_tags(&dir.join("tags.csv")), None);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn read_returns_empty_list_for_empty_file() {
    let dir = scratch_dir("empty");
    std::fs::write(dir.join("tags.csv"), "").unwrap();
    assert_eq!(read_routine_tags(&dir.join("tags.csv")), Some(vec![]));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn read_trims_fields_and_drops_blank_ones() {
    let dir = scratch_dir("trim");
    std::fs::write(dir.join("tags.csv"), "  a , ,b\n").unwrap();
    assert_eq!(
        read_routine_tags(&dir.join("tags.csv")),
        Some(strings(&["a", "b"]))
    );
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn read_returns_none_for_malformed_record() {
    let dir = scratch_dir("malformed");
    std::fs::write(dir.join("tags.csv"), b"\xff\n").unwrap();
    assert_eq!(read_routine_tags(&dir.join("tags.csv")), None);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn read_keeps_quoted_comma_inside_one_tag() {
    let dir = scratch_dir("quoted");
    std::fs::write(dir.join("tags.csv"), "nightly,\"a,b\"\n").unwrap();
    assert_eq!(
        read_routine_tags(&dir.join("tags.csv")),
        Some(strings(&["nightly", "a,b"]))
    );
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn format_writes_plain_tags_as_one_record() {
    assert_eq!(
        format_routine_tags(&strings(&["nightly", "ops"])),
        "nightly,ops\n"
    );
}

#[test]
fn format_quotes_commas_and_quotes() {
    assert_eq!(
        format_routine_tags(&strings(&["a,b", "c\"d"])),
        "\"a,b\",\"c\"\"d\"\n"
    );
}

#[test]
fn format_empty_list_is_empty_string() {
    assert_eq!(format_routine_tags(&[]), "");
}

#[test]
fn format_trims_tags_and_drops_blank_ones() {
    assert_eq!(format_routine_tags(&strings(&[" a ", " "])), "a\n");
}

#[test]
fn format_blank_only_list_is_empty_string() {
    assert_eq!(format_routine_tags(&strings(&["  ", ""])), "");
}

#[test]
fn seed_writes_missing_sidecar_from_legacy_tags() {
    let dir = scratch_dir("seed");
    std::fs::write(
        dir.join("routine.toml"),
        "title = \"Legacy\"\nagent = \"claude\"\ntags = [\"x\"]\n",
    )
    .unwrap();
    seed_tags_csv_from_legacy_toml(&dir);
    assert_eq!(
        std::fs::read_to_string(dir.join("tags.csv")).unwrap(),
        "x\n"
    );
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn seed_preserves_existing_sidecar() {
    let dir = scratch_dir("seed-existing");
    std::fs::write(dir.join("tags.csv"), "keep\n").unwrap();
    std::fs::write(
        dir.join("routine.toml"),
        "title = \"Legacy\"\nagent = \"claude\"\ntags = [\"x\"]\n",
    )
    .unwrap();
    seed_tags_csv_from_legacy_toml(&dir);
    assert_eq!(
        std::fs::read_to_string(dir.join("tags.csv")).unwrap(),
        "keep\n"
    );
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn seed_skips_absent_or_empty_legacy_tags() {
    let dir = scratch_dir("seed-none");
    std::fs::write(
        dir.join("routine.toml"),
        "title = \"Legacy\"\nagent = \"claude\"\ntags = []\n",
    )
    .unwrap();
    seed_tags_csv_from_legacy_toml(&dir);
    assert!(!dir.join("tags.csv").exists());
    std::fs::write(
        dir.join("routine.toml"),
        "title = \"Legacy\"\nagent = \"claude\"\n",
    )
    .unwrap();
    seed_tags_csv_from_legacy_toml(&dir);
    assert!(!dir.join("tags.csv").exists());
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn seed_ignores_directory_without_routine_toml() {
    let dir = scratch_dir("seed-no-toml");
    seed_tags_csv_from_legacy_toml(&dir);
    assert!(!dir.join("tags.csv").exists());
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
#[cfg(unix)]
fn seed_logs_write_failure_and_writes_nothing() {
    use std::os::unix::fs::PermissionsExt;

    let dir = scratch_dir("seed-write-fail");
    std::fs::write(
        dir.join("routine.toml"),
        "title = \"Legacy\"\nagent = \"claude\"\ntags = [\"x\"]\n",
    )
    .unwrap();
    let original_perms = std::fs::metadata(&dir).unwrap().permissions();
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o555)).unwrap();
    seed_tags_csv_from_legacy_toml(&dir);
    std::fs::set_permissions(&dir, original_perms).unwrap();
    assert!(!dir.join("tags.csv").exists());
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn resolve_prefers_tags_csv_over_legacy_on_mismatch() {
    assert_eq!(
        resolve_tags("r", Some(strings(&["b"])), strings(&["a"])),
        strings(&["b"])
    );
}

#[test]
fn resolve_does_not_warn_for_padded_legacy_tags() {
    assert_eq!(
        resolve_tags("r", Some(strings(&["a"])), strings(&[" a ", ""])),
        strings(&["a"])
    );
}

#[test]
fn resolve_uses_tags_csv_when_legacy_key_is_absent() {
    let dir = scratch_dir("resolve-no-legacy");
    std::fs::write(dir.join("tags.csv"), "a\n").unwrap();
    let csv = read_routine_tags(&dir.join("tags.csv"));
    assert_eq!(resolve_tags("r", csv, Vec::new()), strings(&["a"]));
    let _ = std::fs::remove_dir_all(dir);
}
