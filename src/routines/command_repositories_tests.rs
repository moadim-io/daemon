#![allow(
    clippy::missing_docs_in_private_items,
    reason = "test helpers and fixtures do not need doc comments"
)]

use super::*;
use crate::routines::model::Repository;

#[test]
fn repo_dir_name_https_url_strips_git_suffix() {
    assert_eq!(
        repo_dir_name("https://github.com/octocat/Hello-World.git"),
        "Hello-World"
    );
}

#[test]
fn repo_dir_name_scp_style_url() {
    assert_eq!(
        repo_dir_name("git@github.com:octocat/Hello-World"),
        "Hello-World"
    );
}

#[test]
fn repo_dir_name_trailing_slash_ignored() {
    assert_eq!(
        repo_dir_name("https://github.com/octocat/Hello-World/"),
        "Hello-World"
    );
}

#[test]
fn repo_dir_name_sanitizes_unsafe_chars() {
    // Anything outside [A-Za-z0-9._-] becomes '_' so the name is safe to splice unquoted into a
    // double-quoted "$WB/<name>" shell string.
    assert_eq!(repo_dir_name("weird repo name!"), "weird_repo_name_");
}

#[test]
fn repo_dir_name_empty_falls_back() {
    assert_eq!(repo_dir_name(""), "repo");
    assert_eq!(repo_dir_name("/"), "repo");
}

#[test]
fn repo_dir_name_bare_host_uses_host_as_name() {
    // No path segment beyond the host still yields a real (if unusual) name, not the fallback.
    assert_eq!(repo_dir_name("https://example.com/"), "example.com");
}

#[test]
fn clone_repository_stmts_empty_for_no_repositories() {
    assert!(clone_repository_stmts(&[]).is_empty());
}

#[test]
fn clone_repository_stmts_caches_by_url_and_fails_loudly() {
    let repos = vec![Repository {
        repository: "https://github.com/octocat/Hello-World.git".to_string(),
        branch: Some("main".to_string()),
    }];
    let stmts = clone_repository_stmts(&repos);
    assert_eq!(stmts.len(), 1);
    let stmt = &stmts[0];

    let cache = crate::paths::repo_cache_dir("https://github.com/octocat/Hello-World.git")
        .to_string_lossy()
        .into_owned();

    // First run mirror-clones the cache; a later run of any routine sharing this URL only fetches
    // it, reusing already-downloaded objects instead of re-cloning from the remote (#466).
    assert!(
        stmt.contains(&format!(
            "git clone --mirror --quiet {} ",
            shell_quote("https://github.com/octocat/Hello-World.git")
        )),
        "expected a mirror clone fallback in: {stmt}"
    );
    assert!(
        stmt.contains(&format!(
            "git -C {} fetch --prune --quiet origin",
            shell_quote(&cache)
        )),
        "expected a cache fetch in: {stmt}"
    );
    // The workbench copy is a local clone from the cache, at the declared branch.
    assert!(
        stmt.contains(&format!(
            "git clone --local --quiet -b {} {}",
            shell_quote("main"),
            shell_quote(&cache)
        )),
        "expected a branch-scoped local clone from the cache in: {stmt}"
    );
    assert!(
        stmt.contains(r#""$WB/Hello-World""#),
        "expected the workbench target directory in: {stmt}"
    );
    // Both the cache sync and the workbench materialization abort the launch loudly on failure,
    // recording the reason in agent.log — mirroring the existing cp/setup/tmux guards.
    assert!(
        stmt.contains("failed to sync cached repository")
            && stmt.contains(r#"tee -a "$WB/agent.log" >&2; exit 1; }"#),
        "expected a loud abort on cache-sync failure in: {stmt}"
    );
    assert!(
        stmt.contains("failed to materialize repository") && stmt.contains("(branch main)"),
        "expected a loud abort naming the branch on materialize failure in: {stmt}"
    );
}

#[test]
fn clone_repository_stmts_no_branch_omits_dash_b() {
    let repos = vec![Repository {
        repository: "https://github.com/octocat/Hello-World.git".to_string(),
        branch: None,
    }];
    let stmts = clone_repository_stmts(&repos);
    assert!(
        stmts[0].contains("git clone --local --quiet ") && !stmts[0].contains(" -b "),
        "no declared branch must not emit a -b flag: {}",
        stmts[0]
    );
}

#[test]
fn clone_repository_stmts_one_entry_per_repository() {
    let repos = vec![
        Repository {
            repository: "https://github.com/a/a.git".to_string(),
            branch: None,
        },
        Repository {
            repository: "https://github.com/b/b.git".to_string(),
            branch: None,
        },
    ];
    assert_eq!(clone_repository_stmts(&repos).len(), 2);
}

#[test]
fn build_routine_command_embeds_repository_clone_stmts() {
    let routine = {
        let mut routine = make_routine("Cmd Repo Clone Routine");
        routine.repositories = vec![Repository {
            repository: "https://github.com/octocat/Hello-World.git".to_string(),
            branch: Some("main".to_string()),
        }];
        routine
    };
    let agent = AgentCommand {
        command: "claude".to_string(),
        args: vec![],
        setup: None,
        instructions_file: "CLAUDE.md".to_string(),
    };
    let cmd = build_routine_command(&routine, &agent, TriggerSource::Scheduled);
    assert!(
        cmd.contains(r#""$WB/Hello-World""#),
        "expected the repo clone step in the built command: {cmd}"
    );
}

#[test]
fn build_routine_command_no_repositories_emits_no_clone_stmts() {
    let routine = make_routine("Cmd No Repo Routine");
    let agent = AgentCommand {
        command: "claude".to_string(),
        args: vec![],
        setup: None,
        instructions_file: "CLAUDE.md".to_string(),
    };
    let cmd = build_routine_command(&routine, &agent, TriggerSource::Scheduled);
    assert!(
        !cmd.contains("git clone") && !cmd.contains("git fetch"),
        "no declared repositories must emit no clone/fetch statements: {cmd}"
    );
}
