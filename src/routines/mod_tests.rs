#![allow(
    clippy::missing_docs_in_private_items,
    reason = "test helpers and fixtures do not need doc comments"
)]

use super::*;

fn make_routine(id: &str) -> Routine {
    Routine {
        model: None,
        id: id.to_string(),
        schedule: "@daily".to_string(),
        schedules: vec![],
        title: "My Routine".to_string(),
        agent: "claude".to_string(),
        prompt: "do the thing".to_string(),
        goal: None,
        repositories: vec![Repository {
            repository: "https://github.com/octocat/Hello-World".to_string(),
            branch: Some("master".to_string()),
            auto_pull: true,
        }],
        machines: vec![crate::machine::current_machine()],
        enabled: true,
        disabled_reason: None,
        source: "managed".to_string(),
        created_at: 0,
        updated_at: 0,
        last_manual_trigger_at: None,
        last_scheduled_trigger_at: None,
        snoozed_until: None,
        skip_runs: None,
        power_saving: false,
        power_saving_exempt: false,
        tags: vec![],
        ttl_secs: None,
        max_runtime_secs: None,
        env: std::collections::HashMap::new(),
        auto_disabled_reason: None,
        consecutive_failures: 0,
        failure_threshold: None,
        notifications: Default::default(),
    }
}

#[test]
fn slugify_basic() {
    assert_eq!(slugify("My Routine"), "my-routine");
    assert_eq!(slugify("  Hello,  World! "), "hello-world");
    assert_eq!(slugify("UPPER_case-123"), "upper-case-123");
}

#[test]
fn slugify_empty_falls_back() {
    assert_eq!(slugify(""), "routine");
    assert_eq!(slugify("---"), "routine");
    assert_eq!(slugify("!@#$"), "routine");
}

#[test]
fn slugify_preserves_non_ascii_letters() {
    // Hebrew and CJK titles must not collapse to the "routine" fallback (#262).
    assert_eq!(slugify("עדכון יומי"), "עדכון-יומי");
    assert_eq!(slugify("日次レポート"), "日次レポート");
    assert_eq!(slugify("Отчёт"), "отчёт");
    // Latin diacritics are kept rather than silently dropped.
    assert_eq!(slugify("Café Report"), "café-report");
}

#[test]
fn slugify_distinct_non_ascii_titles_produce_distinct_slugs() {
    let slug_one = slugify("עדכון יומי");
    let slug_two = slugify("דוח שבועי");
    assert_ne!(slug_one, "routine");
    assert_ne!(slug_two, "routine");
    assert_ne!(slug_one, slug_two);
}

#[test]
fn compose_prompt_lists_repos_and_prompt() {
    let routine = make_routine("x");
    let prompt = compose_prompt(&routine);
    assert!(prompt.contains("# Workbench"));
    assert!(
        prompt.contains("- ./Hello-World — https://github.com/octocat/Hello-World (branch master)")
    );
    assert!(prompt.contains("do the thing"));
}

#[test]
fn compose_prompt_repo_without_branch() {
    let mut routine = make_routine("x");
    routine.repositories = vec![Repository {
        repository: "git@example.com:a/b".to_string(),
        branch: None,
        auto_pull: true,
    }];
    let prompt = compose_prompt(&routine);
    assert!(prompt.contains("- ./b — git@example.com:a/b\n"));
}

#[test]
fn compose_prompt_without_repositories_omits_clone_header() {
    let mut routine = make_routine("x");
    routine.repositories = vec![];
    let prompt = compose_prompt(&routine);
    assert!(prompt.contains("# Workbench"));
    assert!(prompt.contains("You are working in an empty directory.\n"));
    // No dangling "already cloned" header (and no empty bullet list) when there are no repos.
    assert!(!prompt.contains("already cloned"));
    assert!(!prompt.contains("\n- "));
    assert!(prompt.contains("do the thing"));
}

#[test]
fn compose_prompt_renders_goal_section_when_set() {
    let mut routine = make_routine("x");
    routine.goal = Some("Keep the PR backlog small.".to_string());
    let prompt = compose_prompt(&routine);
    // The goal appears as a `## Goal` section before the `---` prompt separator.
    let goal_at = prompt.find("## Goal").expect("goal section present");
    let sep_at = prompt.find("\n---\n").expect("prompt separator present");
    assert!(goal_at < sep_at, "goal must precede the prompt");
    assert!(prompt.contains("Keep the PR backlog small."));
}

#[test]
fn compose_prompt_omits_goal_section_when_unset_or_blank() {
    let mut routine = make_routine("x");
    routine.goal = None;
    assert!(!compose_prompt(&routine).contains("## Goal"));
    routine.goal = Some("   \n\t".to_string());
    assert!(!compose_prompt(&routine).contains("## Goal"));
}

#[test]
fn compose_prompt_omits_open_flags_section_when_none() {
    let routine = make_routine("x");
    let prompt = compose_prompt(&routine);
    assert!(!prompt.contains("Open flags"));
}

#[test]
fn compose_prompt_includes_open_flags_section() {
    let mut routine = make_routine("x");
    routine.title = "Compose Prompt Flags Test ZZZ".to_string();
    let slug = slugify(&routine.title);
    flags::create_flag(
        &slug,
        "bug",
        "the thing is broken",
        flags::FlagScope::General,
    )
    .unwrap();
    flags::create_flag(&slug, "gap", "missing context", flags::FlagScope::Local).unwrap();

    let prompt = compose_prompt(&routine);
    assert!(prompt.contains("# Open flags"));
    assert!(prompt.contains("**bug** (general): the thing is broken"));
    assert!(prompt.contains("**gap** (local): missing context"));

    crate::routine_storage::remove_routine_dir(&slug).unwrap();
}

#[test]
fn substitute_replaces_placeholders() {
    assert_eq!(
        substitute("read {prompt_file} in {workbench}", ".", "prompt.md"),
        "read prompt.md in ."
    );
    assert_eq!(
        substitute("claude {prompt}", ".", "prompt.md"),
        r#"claude "$(cat prompt.md)""#
    );
}
include!("shell_quote_wraps_and_escapes_tests.rs");
