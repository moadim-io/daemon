//! Auto-pull (#1132): fetch + fast-forward each of a routine's `repositories` into a persistent
//! local cache before every run, so routines that rely on a fresh checkout no longer have to
//! reinvent that sync logic themselves.
//!
//! Best-effort and non-blocking: a repository that fails to clone/fetch/merge (unreachable
//! remote, diverged branch, …) never stops the run — [`sync_repositories`] just returns the
//! failure reason for the caller (`service_trigger::spawn_routine_command`) to raise as an
//! `auto_pull_failed` flag instead of failing silently.
//!
//! ponytail: the cache exists so the daemon can catch pull failures for the operator; the agent's
//! own workbench still clones its own working copy (`command::compose_prompt` still says "clone
//! any you need"). Point the agent at this cache directly instead, if the redundant clone ever
//! matters enough to justify wiring it through the prompt.

use std::path::Path;
use std::process::Command;

use super::command::slugify;
use super::model::{Repository, Routine};
use crate::paths::routine_repo_dir;

/// Resolve the `git` executable to invoke for auto-pull.
///
/// Honours `MOADIM_GIT_BIN` (test seam, mirroring `service_trigger::sh_bin`'s `MOADIM_SH_BIN`).
/// In **test builds**, when no shim is configured this returns a path that cannot exist, so an
/// un-shimmed test fails its sync attempt harmlessly instead of shelling out to a real `git`
/// against a real network.
fn git_bin() -> String {
    if let Ok(bin) = std::env::var("MOADIM_GIT_BIN") {
        return bin;
    }
    #[cfg(test)]
    let fallback = "/nonexistent/moadim-test-git-guard".to_string();
    #[cfg(not(test))]
    let fallback = "git".to_string();
    fallback
}

/// Run `git <args>` (in `dir`, if given), returning a one-line error naming the command and
/// stderr on failure.
fn run_git(dir: Option<&Path>, args: &[&str]) -> Result<(), String> {
    let mut cmd = Command::new(git_bin());
    if let Some(dir) = dir {
        cmd.arg("-C").arg(dir);
    }
    cmd.args(args);
    let joined = args.join(" ");
    let output = cmd.output().map_err(|err| format!("git {joined}: {err}"))?;
    if output.status.success() {
        Ok(())
    } else {
        Err(format!(
            "git {joined}: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ))
    }
}

/// The branch currently checked out in the local clone at `dir` (`git symbolic-ref --short HEAD`).
///
/// Used only when a [`Repository`] pins no explicit branch: the initial clone already checked out
/// whatever the remote's default branch was, so later syncs merge against *that* branch's
/// remote-tracking ref rather than an ambiguous `FETCH_HEAD` (a plain `git fetch origin` updates
/// every remote branch, not just one).
fn current_branch(dir: &Path) -> Result<String, String> {
    let output = Command::new(git_bin())
        .arg("-C")
        .arg(dir)
        .args(["symbolic-ref", "--short", "HEAD"])
        .output()
        .map_err(|err| format!("git symbolic-ref: {err}"))?;
    if !output.status.success() {
        return Err(format!(
            "git symbolic-ref: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Clone-or-update `repository` into its persistent cache dir under routine `slug`.
///
/// A first sync clones (optionally pinned to `repository.branch`, otherwise the remote default).
/// Every later sync fetches, switches to `repository.branch` when it names one (so editing the
/// branch in `routine.toml` after the initial clone still takes effect), then fast-forward merges
/// that branch's remote-tracking ref — never the bare `FETCH_HEAD`, which a plain `git fetch
/// origin` leaves ambiguous once the remote has more than one branch.
fn sync_one(slug: &str, repository: &Repository) -> Result<(), String> {
    let dir = routine_repo_dir(slug, &slugify(&repository.repository));
    if !dir.join(".git").is_dir() {
        let parent = dir.parent().unwrap_or(&dir);
        std::fs::create_dir_all(parent)
            .map_err(|err| format!("creating cache dir {}: {err}", parent.display()))?;
        let dir_str = dir.to_string_lossy().into_owned();
        let mut args = vec!["clone", "--quiet"];
        if let Some(branch) = &repository.branch {
            args.push("--branch");
            args.push(branch);
        }
        args.push(&repository.repository);
        args.push(&dir_str);
        return run_git(None, &args);
    }
    run_git(Some(&dir), &["fetch", "--quiet", "origin"])?;
    let branch = match &repository.branch {
        Some(branch) => {
            run_git(Some(&dir), &["checkout", "--quiet", branch])?;
            branch.clone()
        }
        None => current_branch(&dir)?,
    };
    run_git(
        Some(&dir),
        &["merge", "--ff-only", "--quiet", &format!("origin/{branch}")],
    )
}

/// Sync every repository of `routine` into its persistent cache, returning one error string per
/// repository that failed (empty when every sync succeeded, or `auto_pull` is off, or the routine
/// lists no repositories).
pub(crate) fn sync_repositories(routine: &Routine) -> Vec<String> {
    if !routine.auto_pull {
        return Vec::new();
    }
    let slug = slugify(&routine.title);
    routine
        .repositories
        .iter()
        .filter_map(|repo| {
            sync_one(&slug, repo)
                .err()
                .map(|err| format!("{}: {err}", repo.repository))
        })
        .collect()
}

#[cfg(test)]
#[path = "repo_sync_tests.rs"]
mod repo_sync_tests;
