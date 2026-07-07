//! Routine flags: a lightweight, typed note an agent (or a human, via MCP/HTTP) attaches to a
//! routine mid-run when something is unclear — a gap, a bug, an edge case, a question — with no
//! other channel back to a human (the agent runs unattended inside tmux).
//!
//! Each flag is one file under `{routine_dir}/flags/`, named `{slugify(type)}-{timestamp}.md`
//! (general, committed) or `...timestamp.local.md` (local, gitignored — matches the `*.local.*`
//! pattern already seeded into every routine's `.gitignore`). The file's first line is the exact
//! (unslugified) `type`, then a blank line, then the free-text `description`.
//!
//! There is no status field: an "open" flag is simply a file that exists. Resolving a flag means
//! deleting it ([`resolve_flag`]).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::paths::routine_flags_dir;
use crate::utils::atomic::atomic_write;
use crate::utils::time::now_secs;

use super::command::slugify;

/// Whether a flag file is committed to version control or kept machine-local.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, utoipa::ToSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum FlagScope {
    /// Committed to git (`{type}-{timestamp}.md`).
    General,
    /// Gitignored, machine-local (`{type}-{timestamp}.local.md`).
    Local,
}

impl FlagScope {
    /// The filename suffix (including the leading `.`) this scope's flag files carry.
    fn suffix(self) -> &'static str {
        match self {
            Self::General => ".md",
            Self::Local => ".local.md",
        }
    }
}

/// A single flag raised against a routine.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, utoipa::ToSchema)]
pub struct Flag {
    /// Filename on disk under the routine's `flags/` dir; the handle used to resolve it.
    pub filename: String,
    /// Free-text category, e.g. `"bug"`, `"gap"`, `"edge_case"`, `"question"`.
    #[serde(rename = "type")]
    pub flag_type: String,
    /// Free-text body describing what's unclear.
    pub description: String,
    /// Whether this flag is committed (general) or machine-local.
    pub scope: FlagScope,
    /// Unix timestamp (seconds) the flag was created.
    pub created_at: u64,
}

/// `true` when `filename` is safe to join onto [`routine_flags_dir`] — rejects path separators,
/// `..` traversal, and anything not ending in `.md` (every flag file, local or general, does).
/// Guards [`resolve_flag`] against a caller-supplied filename escaping the flags directory.
fn is_safe_flag_filename(filename: &str) -> bool {
    !filename.is_empty()
        && !filename.contains(['/', '\\'])
        && !filename.contains("..")
        && std::path::Path::new(filename)
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("md"))
}

/// Split a flag filename into its `(created_at, scope)`, or `None` if it doesn't match the
/// `{anything}-{timestamp}(.local)?.md` shape.
///
/// Only the *last* `-`-delimited token before the extension is read (the timestamp); whatever
/// precedes it is the type slug and is not parsed back — the authoritative type lives in the file
/// body, read separately by [`list_flags`].
fn parse_filename(filename: &str) -> Option<(u64, FlagScope)> {
    let (stem, scope) = if let Some(stem) = filename.strip_suffix(".local.md") {
        (stem, FlagScope::Local)
    } else if let Some(stem) = filename.strip_suffix(".md") {
        (stem, FlagScope::General)
    } else {
        return None;
    };
    let (_, ts) = stem.rsplit_once('-')?;
    let created_at = ts.parse().ok()?;
    Some((created_at, scope))
}

/// Create a new flag under the routine identified by `slug`, returning the persisted record.
///
/// `flag_type` and `description` are trimmed; callers are expected to have already rejected blank
/// values (mirroring the other `validate_*`/`reject_*` boundary checks in `service.rs`). On a
/// same-second collision with an existing flag the timestamp is bumped by one second at a time
/// until the filename is free, so a flag never silently overwrites another.
pub fn create_flag(
    slug: &str,
    flag_type: &str,
    description: &str,
    scope: FlagScope,
) -> std::io::Result<Flag> {
    let flag_type = flag_type.trim();
    let description = description.trim();
    let dir = routine_flags_dir(slug);
    crate::utils::fs_perms::create_private_dir_all(&dir)?;

    let type_slug = slugify(flag_type);
    let mut created_at = now_secs();
    let filename = loop {
        let candidate = format!("{type_slug}-{created_at}{}", scope.suffix());
        if !dir.join(&candidate).exists() {
            break candidate;
        }
        created_at += 1;
    };

    atomic_write(
        &dir.join(&filename),
        format!("{flag_type}\n\n{description}\n").as_bytes(),
    )?;

    Ok(Flag {
        filename,
        flag_type: flag_type.to_string(),
        description: description.to_string(),
        scope,
        created_at,
    })
}

/// List every open flag for the routine identified by `slug`, oldest first.
///
/// Returns an empty list when the routine has no `flags/` dir yet (nothing has ever been raised)
/// rather than erroring, mirroring how other routine sidecars are read.
pub fn list_flags(slug: &str) -> Vec<Flag> {
    let dir = routine_flags_dir(slug);
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return Vec::new();
    };
    let mut flags: Vec<Flag> = entries
        .flatten()
        .filter_map(|entry| {
            let filename = entry.file_name().to_string_lossy().into_owned();
            let (created_at, scope) = parse_filename(&filename)?;
            let text = std::fs::read_to_string(entry.path()).ok()?;
            let mut parts = text.splitn(2, "\n\n");
            let flag_type = parts.next().unwrap_or_default().trim().to_string();
            let description = parts.next().unwrap_or_default().trim().to_string();
            Some(Flag {
                filename,
                flag_type,
                description,
                scope,
                created_at,
            })
        })
        .collect();
    flags.sort_by_key(|flag| flag.created_at);
    flags
}

/// Resolve (delete) the flag named `filename` under the routine identified by `slug`.
///
/// Returns `Ok(true)` if a flag was removed, `Ok(false)` if `filename` was unsafe (see
/// [`is_safe_flag_filename`]) or named no existing flag — both read as "nothing to resolve" to the
/// caller, since neither leaves anything on disk to clean up.
pub fn resolve_flag(slug: &str, filename: &str) -> std::io::Result<bool> {
    if !is_safe_flag_filename(filename) {
        return Ok(false);
    }
    let path = routine_flags_dir(slug).join(filename);
    if !path.exists() {
        return Ok(false);
    }
    std::fs::remove_file(&path)?;
    Ok(true)
}

#[cfg(test)]
#[path = "flags_tests.rs"]
mod flags_tests;
