
/// Reject `repositories` entries whose URL (or set branch) is empty/whitespace-only, and return a
/// normalized copy with surrounding whitespace trimmed.
///
/// `repository` is a free-form string rendered verbatim into the agent's `prompt.compiled.local.md` preamble by
/// `compose_prompt` (see #241), so a blank or padded entry yields a broken `- ` clone bullet. An
/// empty list is valid — this only guards the contents of non-empty entries. Mirrors the
/// `validate_cron` / `validate_agent` boundary checks for the other routine fields (#224/#226).
pub(super) fn validate_repositories(repos: &[Repository]) -> Result<Vec<Repository>, AppError> {
    let mut normalized = Vec::with_capacity(repos.len());
    for (index, repo) in repos.iter().enumerate() {
        let repository = repo.repository.trim();
        if repository.is_empty() {
            return Err(AppError::BadRequest(format!(
                "repositories[{index}].repository must not be empty or whitespace-only"
            )));
        }
        let branch = match &repo.branch {
            Some(branch) => {
                let trimmed = branch.trim();
                if trimmed.is_empty() {
                    return Err(AppError::BadRequest(format!(
                        "repositories[{index}].branch must not be empty or whitespace-only when set"
                    )));
                }
                Some(trimmed.to_string())
            }
            None => None,
        };
        normalized.push(Repository {
            repository: repository.to_string(),
            branch,
            auto_pull: repo.auto_pull,
        });
    }
    Ok(normalized)
}

/// Reject blank (empty/whitespace-only) `tags` entries and return a normalized copy with each tag
/// trimmed and duplicates collapsed (first occurrence kept).
///
/// Tags are free-form labels for grouping routines; an empty list is valid. This only guards the
/// contents of non-empty entries, mirroring [`validate_repositories`]: a blank label carries no
/// meaning and would render as an empty chip, so it is refused at edit time rather than stored.
/// The dedup step mirrors [`validate_machines()`]: left unchecked, `["nightly", "nightly"]` (or a
/// padded repeat like `" nightly "`) persists and renders as a doubled chip in the routine row and
/// an inflated (if harmless) entry in the tag facet's per-tag matching, for a label that names one
/// concept once.
pub(super) fn validate_tags(tags: &[String]) -> Result<Vec<String>, AppError> {
    let mut normalized: Vec<String> = Vec::with_capacity(tags.len());
    for (index, tag) in tags.iter().enumerate() {
        let trimmed = tag.trim();
        if trimmed.is_empty() {
            return Err(AppError::BadRequest(format!(
                "tags[{index}] must not be empty or whitespace-only"
            )));
        }
        if !normalized.iter().any(|existing| existing == trimmed) {
            normalized.push(trimmed.to_string());
        }
    }
    Ok(normalized)
}

/// Reject an `env` map with an invalid key or a value that could inject an extra shell statement.
///
/// Keys must be POSIX-portable shell identifiers ([`is_valid_env_key`]) —
/// [`crate::routines::command::build_routine_command`]
/// emits each entry as a literal `export KEY=<shell-quoted value>` statement, so a key outside that
/// shape (e.g. containing `=`, whitespace, or `;`) would either fail to export or, unquoted as it
/// must be for `export NAME=...` syntax to work, let a crafted key break out of the statement.
/// Values are shell-quoted ([`crate::routines::command::shell_quote`]) so most characters are safe,
/// but a value containing a newline still splits the single-line, `;`-joined launch command into
/// two shell statements — an injection distinct from anything quoting can neutralize — so newlines
/// are rejected outright (#408).
pub(super) fn validate_env(
    env: &std::collections::HashMap<String, String>,
) -> Result<(), AppError> {
    // Iterate in sorted key order so the *first* error reported is deterministic when several
    // entries are invalid (`HashMap` iteration order varies between runs).
    let mut entries: Vec<_> = env.iter().collect();
    entries.sort_by_key(|&(key, _)| key);
    for (key, value) in entries {
        if !is_valid_env_key(key) {
            return Err(AppError::BadRequest(format!(
                "env key {key:?} is invalid; keys must match [A-Za-z_][A-Za-z0-9_]*"
            )));
        }
        if value.contains('\n') || value.contains('\r') {
            return Err(AppError::BadRequest(format!(
                "env value for key {key:?} must not contain newline characters"
            )));
        }
    }
    Ok(())
}
