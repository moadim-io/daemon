//! Pre-clone/fetch each of a routine's declared `repositories` into the workbench before the
//! agent launches, backed by a persistent local mirror cache — split out of `command.rs` to keep
//! that file under the line-count gate. See issue #466.

use super::shell_quote;
use crate::routines::model::Repository;

/// Directory name a repository's working copy is cloned into under the workbench: the last path
/// segment of its URL (after the final `/` or `:`), minus a trailing `.git`, with every byte
/// outside `[A-Za-z0-9._-]` replaced by `_` so the result is safe to splice, unquoted, into the
/// double-quoted `"$WB/<name>"` shell strings [`clone_repository_stmts`] emits.
///
/// Two declared repositories that happen to share a basename (e.g. `github.com/a/x` and
/// `example.com/b/x`) collide on this name: the second `git clone` then fails because the target
/// directory already holds the first repository, which aborts the launch the same way any other
/// clone failure does (see [`clone_repository_stmts`]) rather than silently overwriting one with
/// the other.
pub(crate) fn repo_dir_name(url: &str) -> String {
    let trimmed = url.trim_end_matches('/');
    let last = trimmed.rsplit(['/', ':']).next().unwrap_or(trimmed);
    let stem = last.strip_suffix(".git").unwrap_or(last);
    let sanitized: String = stem
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '.' | '-' | '_') {
                ch
            } else {
                '_'
            }
        })
        .collect();
    if sanitized.is_empty() {
        "repo".to_string()
    } else {
        sanitized
    }
}

/// Build the shell statements that materialize `repositories` into the workbench (`$WB`) before
/// the agent launches, instead of leaving an empty directory and a "clone any you need" prose
/// instruction for the agent to (re-)act on every single fire (#466).
///
/// Each repository is backed by a persistent local mirror at
/// [`crate::paths::repo_cache_dir`], keyed by its exact URL: the first run clones the mirror with
/// `git clone --mirror`, every later run — of any routine referencing that same URL — only
/// `git fetch`es it, so repeated fires reuse already-downloaded objects instead of re-cloning from
/// the remote. The workbench copy is then a fast, local `git clone` *from that mirror* (no
/// network), checked out at the declared `branch` when set.
///
/// A failure at either step — a bad URL, an unreachable host, or a `branch` that does not exist
/// upstream — aborts the run with the reason recorded in `agent.log`, mirroring the existing
/// `cp prompt.md`/`setup`/`tmux` guards in [`super::build_routine_command`], rather than the
/// failure surfacing only inside the agent session (or not at all).
pub(crate) fn clone_repository_stmts(repositories: &[Repository]) -> Vec<String> {
    repositories
        .iter()
        .map(|repo| {
            let url = shell_quote(&repo.repository);
            let cache = shell_quote(&crate::paths::repo_cache_dir(&repo.repository).to_string_lossy());
            let dir_name = repo_dir_name(&repo.repository);
            let branch_arg = match &repo.branch {
                Some(branch) => format!(" -b {}", shell_quote(branch)),
                None => String::new(),
            };
            let branch_desc = match &repo.branch {
                Some(branch) => format!(" (branch {branch})"),
                None => String::new(),
            };
            format!(
                r#"mkdir -p {cache_parent} && {{ if [ -d {cache} ]; then git -C {cache} fetch --prune --quiet origin; else git clone --mirror --quiet {url} {cache}; fi; }} || {{ echo "moadim: failed to sync cached repository {url}; aborting launch" | tee -a "$WB/agent.log" >&2; exit 1; }}; git clone --local --quiet{branch_arg} {cache} "$WB/{dir_name}" || {{ echo "moadim: failed to materialize repository {url}{branch_desc}; aborting launch" | tee -a "$WB/agent.log" >&2; exit 1; }}"#,
                cache_parent =
                    shell_quote(&crate::paths::config_dir().join("cache").to_string_lossy()),
            )
        })
        .collect()
}
