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
