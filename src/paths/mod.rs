//! Path builders for moadim config directories.

use std::path::PathBuf;

/// Returns the path to `~/.config/moadim/`.
pub fn config_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".config")
        .join("moadim")
}
