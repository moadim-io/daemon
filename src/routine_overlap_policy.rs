//! Read/write support for a routine's tracked overlap policy sidecar.

use crate::utils::atomic::atomic_write;
use serde::{Deserialize, Serialize};

/// Current on-disk schema version for a routine overlap policy.
const OVERLAP_POLICY_VERSION: u32 = 1;

/// Serialized, versioned opt-in that lives beside a routine's TOML.
#[derive(Deserialize, Serialize)]
struct OverlapPolicy {
    /// Schema version, used to fail closed on a future incompatible policy.
    version: u32,
    /// Whether the daemon may launch another fire while a prior fire is live.
    allow_overlapping_runs: bool,
}

/// Read the effective overlap policy for a routine directory.
///
/// An absent sidecar preserves the safe default of denying overlapping fires. A malformed sidecar
/// is returned as an error so callers can fail closed instead of accidentally permitting overlap.
pub(crate) fn read_overlap_policy(rel_dir: &str) -> Result<bool, String> {
    let path = crate::paths::routine_overlap_json_path(rel_dir);
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(format!("cannot read {}: {error}", path.display())),
    };
    let policy: OverlapPolicy = serde_json::from_str(&text)
        .map_err(|error| format!("invalid overlap policy {}: {error}", path.display()))?;
    if policy.version != OVERLAP_POLICY_VERSION {
        return Err(format!(
            "unsupported overlap policy version {} in {}",
            policy.version,
            path.display()
        ));
    }
    Ok(policy.allow_overlapping_runs)
}

/// Create a missing policy sidecar with the safe default without replacing an explicit policy.
///
/// Routine writes run on creation, updates, and startup re-persistence, so this also materializes
/// the policy file for routines created by older daemon versions.
pub(crate) fn ensure_overlap_policy(rel_dir: &str) -> std::io::Result<()> {
    let path = crate::paths::routine_overlap_json_path(rel_dir);
    if path.exists() {
        return Ok(());
    }
    write_overlap_policy_file(&path, false)
}

/// Serialize and atomically replace one explicit overlap policy file.
fn write_overlap_policy_file(
    path: &std::path::Path,
    allow_overlapping_runs: bool,
) -> std::io::Result<()> {
    let policy = OverlapPolicy {
        version: OVERLAP_POLICY_VERSION,
        allow_overlapping_runs,
    };
    let bytes = serde_json::to_vec(&policy).map_err(std::io::Error::other)?;
    atomic_write(path, &bytes)
}

/// Persist the overlap policy for a routine directory.
///
/// `false` removes the sidecar only in tests that need to exercise the legacy missing-file path.
#[cfg(test)]
pub(crate) fn write_overlap_policy(
    rel_dir: &str,
    allow_overlapping_runs: bool,
) -> std::io::Result<()> {
    let path = crate::paths::routine_overlap_json_path(rel_dir);
    if !allow_overlapping_runs {
        if path.exists() {
            std::fs::remove_file(path)?;
        }
        return Ok(());
    }
    write_overlap_policy_file(&path, allow_overlapping_runs)
}
