//! `moadim export`: snapshot the tracked config tree into a single portable JSON bundle.

use std::collections::BTreeMap;
use std::io;
use std::path::{Path, PathBuf};

use super::bundle::{is_tracked_rel_path, Bundle, BUNDLE_VERSION};

/// Run `moadim export`: collect every tracked file under the config directory into a
/// [`Bundle`] and write it as pretty-printed JSON to `out` (atomically), or to stdout when `out`
/// is `None`. Returns the process exit code (`0` on success, `1` on failure).
pub(crate) fn run_export(out: Option<PathBuf>) -> i32 {
    let root = crate::paths::config_dir();
    let bundle = match collect_bundle(&root) {
        Ok(bundle) => bundle,
        Err(err) => {
            eprintln!("export failed: cannot read {}: {err}", root.display());
            return 1;
        }
    };
    let count = bundle.files.len();
    let json = to_json(&bundle);
    let Some(path) = out else {
        println!("{json}");
        return 0;
    };
    match crate::utils::atomic::atomic_write(&path, json.as_bytes()) {
        Ok(()) => {
            println!("exported {count} tracked file(s) to {}", path.display());
            0
        }
        Err(err) => {
            eprintln!("export failed: cannot write {}: {err}", path.display());
            1
        }
    }
}

/// Serialize `bundle` as pretty-printed JSON.
///
/// Serializing a struct of a `u32` and a string→string map cannot fail, so the error arm maps to
/// an empty string rather than threading an impossible `Result` to the caller.
fn to_json(bundle: &Bundle) -> String {
    serde_json::to_string_pretty(bundle).unwrap_or_default()
}

/// Walk the config tree rooted at `root` and collect every tracked, UTF-8 file into a [`Bundle`].
fn collect_bundle(root: &Path) -> io::Result<Bundle> {
    let mut files = BTreeMap::new();
    collect_dir(root, Path::new(""), &mut files)?;
    Ok(Bundle {
        version: BUNDLE_VERSION,
        files,
    })
}

/// Recursively walk `dir` (which sits at `rel` relative to the config root), adding every tracked
/// file to `files`.
fn collect_dir(dir: &Path, rel: &Path, files: &mut BTreeMap<String, String>) -> io::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let child_rel = rel.join(entry.file_name());
        let path = entry.path();
        if path.is_dir() {
            collect_dir(&path, &child_rel, files)?;
        } else if is_tracked_rel_path(&child_rel) {
            insert_file(&path, &child_rel, files);
        }
    }
    Ok(())
}

/// Read `path` and store its contents under the POSIX form of `rel`. A tracked file that cannot
/// be read (or is not valid UTF-8) is skipped with a warning rather than aborting the whole
/// export, so one bad file doesn't block backing up everything else.
fn insert_file(path: &Path, rel: &Path, files: &mut BTreeMap<String, String>) {
    match std::fs::read_to_string(path) {
        Ok(content) => {
            files.insert(rel_key(rel), content);
        }
        Err(err) => eprintln!("skipping {}: {err}", path.display()),
    }
}

/// Render `rel` with `/` separators so bundle keys are identical across platforms.
fn rel_key(rel: &Path) -> String {
    let parts: Vec<&str> = rel
        .components()
        .filter_map(|component| component.as_os_str().to_str())
        .collect();
    parts.join("/")
}

#[cfg(test)]
#[path = "export_tests.rs"]
mod export_tests;
