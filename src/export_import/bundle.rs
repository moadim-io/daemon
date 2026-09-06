//! The portable bundle format shared by `moadim export` and `moadim import`, plus the predicate
//! deciding which config-dir-relative paths belong in a bundle.

use std::collections::BTreeMap;
use std::path::{Component, Path};

use serde::{Deserialize, Serialize};

/// Version stamp written into every exported bundle so a future format change can be detected on
/// import instead of silently misreading old or new bundles.
pub(crate) const BUNDLE_VERSION: u32 = 1;

/// A portable snapshot of the tracked moadim config: a map from config-dir-relative POSIX path
/// (`/`-separated on every platform) to that file's UTF-8 contents.
#[derive(Serialize, Deserialize)]
pub(crate) struct Bundle {
    /// Bundle format version; import rejects bundles whose version is not [`BUNDLE_VERSION`].
    pub(crate) version: u32,
    /// Config-dir-relative path → file contents. A `BTreeMap` keeps export output deterministic.
    pub(crate) files: BTreeMap<String, String>,
}

/// True when `rel` (a config-dir-relative path) is one of the tracked files export backs up and
/// import is allowed to write.
///
/// This doubles as import's safety gate: any path with a non-plain component (absolute, `..`, a
/// prefix, or non-UTF-8) is rejected, so a hostile bundle cannot escape the config directory, and
/// gitignored runtime files (`*.local.*`, logs, pids, compiled outputs) are never written because
/// they simply aren't in the allow-list.
pub(crate) fn is_tracked_rel_path(rel: &Path) -> bool {
    let Some(parts) = plain_components(rel) else {
        return false;
    };
    match parts.as_slice() {
        ["notifications.toml" | "user_prompt.md"] => true,
        ["agents", file] => {
            Path::new(file).extension() == Some(std::ffi::OsStr::new("toml"))
                && !file.contains(".local.")
        }
        ["routines", middle @ .., file] => is_tracked_routine_file(middle, file),
        _ => false,
    }
}

/// True when `file`, nested under `routines/` behind the intermediate directories `middle`
/// (routine folders, and optionally the grouping folders of a foldered routine), is one of the
/// tracked per-routine files.
fn is_tracked_routine_file(middle: &[&str], file: &str) -> bool {
    match file {
        // Tracked routine metadata lives directly in a routine directory, so at least one
        // intermediate directory (the routine's folder) must be present.
        "routine.toml" | "schedule.cron" | "disabled.json" => !middle.is_empty(),
        // The pure prompt lives in the routine directory's `prompts/` subfolder.
        "prompt.pure.md" => middle.len() >= 2 && middle.last() == Some(&"prompts"),
        _ => false,
    }
}

/// Split `rel` into its plain ([`Component::Normal`]) UTF-8 components, or `None` when any
/// component is something else (root, prefix, `.`/`..`) or not valid UTF-8 — i.e. when the path
/// cannot be treated as a safe config-dir-relative path.
fn plain_components(rel: &Path) -> Option<Vec<&str>> {
    rel.components()
        .map(|component| match component {
            Component::Normal(part) => part.to_str(),
            _ => None,
        })
        .collect()
}

#[cfg(test)]
#[path = "bundle_tests.rs"]
mod bundle_tests;
