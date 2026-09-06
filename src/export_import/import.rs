//! `moadim import`: validate a bundle produced by `moadim export` and restore its files into the
//! config tree.

use std::path::Path;

use super::bundle::{is_tracked_rel_path, Bundle, BUNDLE_VERSION};

/// What import would do (or did) with one bundled file, decided by whether the target already
/// exists and whether `--force` was given.
enum Action {
    /// The target does not exist yet and will be written.
    Create,
    /// The target exists and `--force` replaces it.
    Overwrite,
    /// The target exists and is left untouched (the default collision behavior).
    Skip,
}

impl Action {
    /// The plan/summary verb printed for this action.
    const fn verb(&self) -> &'static str {
        match self {
            Self::Create => "create",
            Self::Overwrite => "overwrite",
            Self::Skip => "skip (exists)",
        }
    }
}

/// Run `moadim import`: read the bundle at `file`, validate it in full (version, path safety,
/// `routine.toml` parseability) before touching disk, then restore its files under the config
/// directory. Existing files are skipped unless `force` is set; `dry_run` prints the plan and
/// writes nothing. Returns the process exit code (`0` success, `1` write failure, `2` invalid
/// bundle).
pub(crate) fn run_import(file: &Path, dry_run: bool, force: bool) -> i32 {
    let bundle = match read_bundle(file) {
        Ok(bundle) => bundle,
        Err(message) => {
            eprintln!("import failed: {message}");
            return 2;
        }
    };
    let root = crate::paths::config_dir();
    let plan = plan_actions(&root, &bundle, force);
    if dry_run {
        for (rel, _content, action) in &plan {
            println!("{} {rel}", action.verb());
        }
        println!("dry run: nothing written");
        return 0;
    }
    apply(&root, &plan)
}

/// Read and fully validate the bundle at `file`, returning a human-readable error message when
/// anything is off. Validation happens before any write, so a bad bundle leaves the config tree
/// untouched.
fn read_bundle(file: &Path) -> Result<Bundle, String> {
    let raw = std::fs::read_to_string(file)
        .map_err(|err| format!("cannot read {}: {err}", file.display()))?;
    let bundle: Bundle = serde_json::from_str(&raw)
        .map_err(|err| format!("{} is not a moadim export bundle: {err}", file.display()))?;
    if bundle.version != BUNDLE_VERSION {
        return Err(format!(
            "unsupported bundle version {} (this moadim understands version {BUNDLE_VERSION})",
            bundle.version
        ));
    }
    for (rel, content) in &bundle.files {
        validate_entry(rel, content)?;
    }
    Ok(bundle)
}

/// Validate one bundle entry: the path must be a tracked config-dir-relative path (which also
/// rules out traversal outside the config dir), and a `routine.toml` payload must at least be
/// valid TOML so import can't plant files the routine loader immediately chokes on.
fn validate_entry(rel: &str, content: &str) -> Result<(), String> {
    if !is_tracked_rel_path(Path::new(rel)) {
        return Err(format!("bundle entry {rel} is not a tracked config path"));
    }
    if rel.ends_with("routine.toml") {
        toml::from_str::<toml::Value>(content)
            .map_err(|err| format!("bundle entry {rel} is not valid TOML: {err}"))?;
    }
    Ok(())
}

/// Pair every bundled file with the [`Action`] import will take on it: create missing targets,
/// and overwrite or skip existing ones depending on `force`.
fn plan_actions<'bundle>(
    root: &Path,
    bundle: &'bundle Bundle,
    force: bool,
) -> Vec<(&'bundle str, &'bundle str, Action)> {
    bundle
        .files
        .iter()
        .map(|(rel, content)| {
            let action = if root.join(rel).exists() {
                if force {
                    Action::Overwrite
                } else {
                    Action::Skip
                }
            } else {
                Action::Create
            };
            (rel.as_str(), content.as_str(), action)
        })
        .collect()
}

/// Write every planned `Create`/`Overwrite` entry through the atomic-write primitive the storage
/// layer uses, then print a summary. Returns `0` on success, `1` on the first write failure.
fn apply(root: &Path, plan: &[(&str, &str, Action)]) -> i32 {
    let mut written = 0_usize;
    let mut skipped = 0_usize;
    for (rel, content, action) in plan {
        if matches!(action, Action::Skip) {
            println!("{} {rel}", action.verb());
            skipped += 1;
            continue;
        }
        if let Err(err) = write_file(&root.join(rel), content) {
            eprintln!("import failed: cannot write {rel}: {err}");
            return 1;
        }
        println!("{} {rel}", action.verb());
        written += 1;
    }
    println!(
        "imported {written} file(s), skipped {skipped} — run `moadim restart` to resync the crontab"
    );
    0
}

/// Create `target`'s parent directories and write `content` atomically.
///
/// Every import target is `config_dir().join(rel)` with a non-empty relative path, so `parent()`
/// is always present; the `.` fallback only guards against a bare-filename `target`.
fn write_file(target: &Path, content: &str) -> std::io::Result<()> {
    std::fs::create_dir_all(target.parent().unwrap_or(Path::new(".")))?;
    crate::utils::atomic::atomic_write(target, content.as_bytes())
}

#[cfg(test)]
#[path = "import_tests.rs"]
mod import_tests;
