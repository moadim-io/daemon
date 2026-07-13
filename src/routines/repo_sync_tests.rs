#![allow(
    clippy::missing_docs_in_private_items,
    reason = "test helpers and fixtures do not need doc comments"
)]

use super::*;
use crate::routines::model::Repository;
use std::path::PathBuf;

/// Point `MOADIM_HOME_OVERRIDE` at a fresh temp home and `MOADIM_GIT_BIN` at the real `git` on
/// `PATH`, for the duration of a test — restoring/removing both on drop. Tests in this crate run
/// single-threaded (`RUST_TEST_THREADS=1`), so mutating process-global env vars is safe.
struct TestEnv(PathBuf);

impl TestEnv {
    fn set() -> Self {
        let dir = std::env::temp_dir().join(format!("moadim-reposync-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).expect("create temp home");
        // SAFETY: single-threaded test execution (RUST_TEST_THREADS=1).
        unsafe {
            std::env::set_var("MOADIM_HOME_OVERRIDE", &dir);
            std::env::set_var("MOADIM_GIT_BIN", "git");
        }
        Self(dir)
    }
}

impl Drop for TestEnv {
    fn drop(&mut self) {
        // SAFETY: single-threaded test execution (RUST_TEST_THREADS=1).
        unsafe {
            std::env::remove_var("MOADIM_HOME_OVERRIDE");
            std::env::remove_var("MOADIM_GIT_BIN");
        }
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Like [`TestEnv`], but deliberately leaves `MOADIM_GIT_BIN` unset — for the one test that
/// exercises `git_bin()`'s test-only guard (no shim configured).
struct HomeOnly(PathBuf);

impl HomeOnly {
    fn set() -> Self {
        let dir =
            std::env::temp_dir().join(format!("moadim-reposync-guard-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).expect("create temp home");
        // SAFETY: single-threaded test execution (RUST_TEST_THREADS=1).
        unsafe {
            std::env::set_var("MOADIM_HOME_OVERRIDE", &dir);
        }
        Self(dir)
    }
}

impl Drop for HomeOnly {
    fn drop(&mut self) {
        // SAFETY: single-threaded test execution (RUST_TEST_THREADS=1).
        unsafe {
            std::env::remove_var("MOADIM_HOME_OVERRIDE");
        }
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Run `git <args>` in `dir`, panicking on failure — a test fixture helper, not the code under
/// test (which is [`super::run_git`]).
///
/// Ignores the developer's global/system git config (`GIT_CONFIG_GLOBAL`/`GIT_CONFIG_SYSTEM`
/// pointed at `/dev/null`) so these disposable fixture repos never depend on — or are broken by —
/// host-specific settings (commit signing, custom hooks, aliases).
///
/// Also clears `GIT_DIR`/`GIT_WORK_TREE`/`GIT_INDEX_FILE`/`GIT_COMMON_DIR`: when running under
/// `git push`'s pre-push hook, `git` sets these to point at *this* repository for the hook's whole
/// process tree, and they override `-C` — without clearing them, every "isolated" fixture command
/// below silently operates on the real repo instead of `dir`, corrupting it (see the incident this
/// comment documents: two rogue commits landed on the feature branch this test suite was written
/// for, #1132, before this fix). `dir` is always an absolute tempdir path.
fn git(dir: &std::path::Path, args: &[&str]) {
    let status = std::process::Command::new("git")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_SYSTEM", "/dev/null")
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .env_remove("GIT_INDEX_FILE")
        .env_remove("GIT_COMMON_DIR")
        .arg("-C")
        .arg(dir)
        .args(args)
        .status()
        .expect("git available on PATH for tests");
    assert!(status.success(), "git {args:?} failed in {dir:?}");
}

/// Initialize a throwaway local "remote" repo at `dir` with one commit on `main`.
fn init_remote(dir: &std::path::Path) {
    std::fs::create_dir_all(dir).unwrap();
    git(dir, &["init", "--quiet", "-b", "main"]);
    git(dir, &["config", "user.email", "test@example.com"]);
    git(dir, &["config", "user.name", "Test"]);
    std::fs::write(dir.join("f.txt"), "v1\n").unwrap();
    git(dir, &["add", "."]);
    git(dir, &["commit", "--quiet", "-m", "init"]);
}

fn routine_with(repositories: Vec<Repository>, auto_pull: bool) -> Routine {
    Routine {
        id: "r1".to_string(),
        schedule: "@daily".to_string(),
        title: "Repo Sync Test".to_string(),
        agent: "claude".to_string(),
        model: None,
        prompt: "do it".to_string(),
        goal: None,
        repositories,
        auto_pull,
        machines: vec![],
        enabled: true,
        source: "managed".to_string(),
        created_at: 0,
        updated_at: 0,
        last_manual_trigger_at: None,
        last_scheduled_trigger_at: None,
        snoozed_until: None,
        skip_runs: None,
        power_saving: false,
        ttl_secs: None,
        max_runtime_secs: None,
        tags: vec![],
    }
}

#[test]
fn sync_repositories_skips_when_auto_pull_disabled() {
    let _env = TestEnv::set();
    let repo = Repository {
        repository: "/nonexistent/does-not-matter".to_string(),
        branch: None,
    };
    let routine = routine_with(vec![repo], false);
    assert!(sync_repositories(&routine).is_empty());
}

#[test]
fn sync_repositories_empty_when_no_repositories() {
    let _env = TestEnv::set();
    let routine = routine_with(vec![], true);
    assert!(sync_repositories(&routine).is_empty());
}

#[test]
fn sync_repositories_clones_then_fast_forwards_default_branch() {
    let env = TestEnv::set();
    let remote = env.0.join("remote");
    init_remote(&remote);

    let repo = Repository {
        repository: remote.to_string_lossy().into_owned(),
        branch: None,
    };
    let routine = routine_with(vec![repo], true);

    // First sync: clones (remote default branch, "main").
    assert_eq!(sync_repositories(&routine), Vec::<String>::new());
    let cache = routine_repo_dir("repo-sync-test", &slugify(&remote.to_string_lossy()));
    assert!(cache.join(".git").is_dir());
    assert_eq!(
        std::fs::read_to_string(cache.join("f.txt")).unwrap(),
        "v1\n"
    );

    // Push a second commit to the remote, then sync again: fetch + fast-forward merge.
    std::fs::write(remote.join("f.txt"), "v2\n").unwrap();
    git(&remote, &["add", "."]);
    git(&remote, &["commit", "--quiet", "-m", "second"]);

    assert_eq!(sync_repositories(&routine), Vec::<String>::new());
    assert_eq!(
        std::fs::read_to_string(cache.join("f.txt")).unwrap(),
        "v2\n"
    );
}

#[test]
fn sync_repositories_switches_branch_on_a_later_sync() {
    let env = TestEnv::set();
    let remote = env.0.join("remote");
    init_remote(&remote);
    git(&remote, &["checkout", "--quiet", "-b", "feature"]);
    std::fs::write(remote.join("f.txt"), "feature-1\n").unwrap();
    git(&remote, &["add", "."]);
    git(&remote, &["commit", "--quiet", "-m", "feature commit"]);
    git(&remote, &["checkout", "--quiet", "main"]);

    let repo_url = remote.to_string_lossy().into_owned();
    let cache = routine_repo_dir("repo-sync-test", &slugify(&repo_url));

    // Initial sync with no pinned branch clones "main" (the remote default).
    let routine = routine_with(
        vec![Repository {
            repository: repo_url.clone(),
            branch: None,
        }],
        true,
    );
    assert_eq!(sync_repositories(&routine), Vec::<String>::new());
    assert_eq!(
        std::fs::read_to_string(cache.join("f.txt")).unwrap(),
        "v1\n"
    );

    // Re-sync pinned to "feature": checks out and fast-forwards onto it.
    let routine = routine_with(
        vec![Repository {
            repository: repo_url,
            branch: Some("feature".to_string()),
        }],
        true,
    );
    assert_eq!(sync_repositories(&routine), Vec::<String>::new());
    assert_eq!(
        std::fs::read_to_string(cache.join("f.txt")).unwrap(),
        "feature-1\n"
    );
}

#[test]
fn sync_repositories_reports_error_for_each_failing_repo() {
    let env = TestEnv::set();
    let repo = Repository {
        repository: env.0.join("no-such-remote").to_string_lossy().into_owned(),
        branch: None,
    };
    let routine = routine_with(vec![repo.clone(), repo], true);
    let errors = sync_repositories(&routine);
    assert_eq!(errors.len(), 2);
    for err in errors {
        assert!(err.starts_with(&routine.repositories[0].repository));
    }
}

#[test]
fn sync_one_fails_safely_without_a_configured_git_bin() {
    // `HomeOnly` (unlike `TestEnv`) leaves `MOADIM_GIT_BIN` unset: `git_bin()`'s test-only guard
    // must return a nonexistent path rather than falling back to a real `git`, so this never
    // touches the network or the developer's machine.
    let _env = HomeOnly::set();

    let repo = Repository {
        repository: "https://example.invalid/owner/repo".to_string(),
        branch: None,
    };
    let routine = routine_with(vec![repo], true);
    let errors = sync_repositories(&routine);
    assert_eq!(errors.len(), 1);
}

#[test]
fn current_branch_fails_safely_without_a_configured_git_bin() {
    let env = HomeOnly::set();
    assert!(current_branch(&env.0).is_err());
}

#[test]
fn current_branch_errors_on_a_non_git_directory() {
    let env = TestEnv::set();
    let not_a_repo = env.0.join("not-a-repo");
    std::fs::create_dir_all(&not_a_repo).unwrap();
    assert!(current_branch(&not_a_repo).is_err());
}

#[test]
fn sync_one_reports_create_dir_all_failure() {
    let _env = TestEnv::set();
    let slug = "repo-sync-test";
    // Pre-create the `repos/` parent as a *file* so `create_dir_all` for a fresh clone's parent
    // fails (a path component already exists and is not a directory).
    let repos_dir = crate::paths::routine_dir(slug).join("repos");
    std::fs::create_dir_all(repos_dir.parent().unwrap()).unwrap();
    std::fs::write(&repos_dir, "not a directory").unwrap();

    let repo = Repository {
        repository: "/nonexistent/wont-be-reached".to_string(),
        branch: None,
    };
    let routine = routine_with(vec![repo], true);
    let errors = sync_repositories(&routine);
    assert_eq!(errors.len(), 1);
    assert!(errors[0].contains("creating cache dir"));
}

#[test]
fn sync_one_reports_fetch_failure_once_remote_is_gone() {
    let env = TestEnv::set();
    let remote = env.0.join("remote");
    init_remote(&remote);
    let repo = Repository {
        repository: remote.to_string_lossy().into_owned(),
        branch: None,
    };
    let routine = routine_with(vec![repo], true);
    assert_eq!(sync_repositories(&routine), Vec::<String>::new());

    // The clone now exists locally; remove the remote so the next fetch fails.
    std::fs::remove_dir_all(&remote).unwrap();
    let errors = sync_repositories(&routine);
    assert_eq!(errors.len(), 1);
}

#[test]
fn sync_one_reports_checkout_failure_for_an_unknown_branch() {
    let env = TestEnv::set();
    let remote = env.0.join("remote");
    init_remote(&remote);
    let repo_url = remote.to_string_lossy().into_owned();

    // Initial clone (default branch).
    let routine = routine_with(
        vec![Repository {
            repository: repo_url.clone(),
            branch: None,
        }],
        true,
    );
    assert_eq!(sync_repositories(&routine), Vec::<String>::new());

    // Re-sync pinned to a branch that doesn't exist on the remote.
    let routine = routine_with(
        vec![Repository {
            repository: repo_url,
            branch: Some("no-such-branch".to_string()),
        }],
        true,
    );
    let errors = sync_repositories(&routine);
    assert_eq!(errors.len(), 1);
}

#[test]
fn sync_one_reports_current_branch_failure_on_a_detached_clone() {
    let env = TestEnv::set();
    let remote = env.0.join("remote");
    init_remote(&remote);
    let repo_url = remote.to_string_lossy().into_owned();

    let routine = routine_with(
        vec![Repository {
            repository: repo_url.clone(),
            branch: None,
        }],
        true,
    );
    assert_eq!(sync_repositories(&routine), Vec::<String>::new());

    // Detach the cached clone's HEAD: `symbolic-ref` (which `current_branch` relies on to find
    // the branch to merge when no explicit branch is pinned) fails on a detached HEAD.
    let cache = routine_repo_dir("repo-sync-test", &slugify(&repo_url));
    git(&cache, &["checkout", "--quiet", "HEAD~0"]);

    let errors = sync_repositories(&routine);
    assert_eq!(errors.len(), 1);
}
