#![allow(
    clippy::missing_docs_in_private_items,
    reason = "test helpers and fixtures do not need doc comments"
)]

use super::*;
use std::collections::HashMap;

/// Point `MOADIM_HOME_OVERRIDE` at a fresh tempdir for the duration of `body`, restoring the
/// previous value afterwards — mirrors `command_tests::build_routine_command_workbench_base_tracks_moadim_home_override`.
/// Needed here because `env_export_stmts` reads a routine's `routine.local.toml` sidecar from the
/// real `paths::routine_local_toml_path`, which resolves under this override.
fn with_home_override(body: impl FnOnce(&std::path::Path)) {
    let dir = std::env::temp_dir().join(format!("moadim-cmd-env-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let previous = std::env::var_os("MOADIM_HOME_OVERRIDE");
    // SAFETY: single-threaded test harness (`RUST_TEST_THREADS=1`); restored immediately below.
    unsafe {
        std::env::set_var("MOADIM_HOME_OVERRIDE", &dir);
    }
    body(&dir);
    // SAFETY: see above.
    unsafe {
        match previous {
            Some(prev) => std::env::set_var("MOADIM_HOME_OVERRIDE", prev),
            None => std::env::remove_var("MOADIM_HOME_OVERRIDE"),
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// Write `routine.local.toml`'s `[env]` table for `slug` under the current `MOADIM_HOME_OVERRIDE`.
fn write_local_env(slug: &str, entries: &[(&str, &str)]) {
    let path = crate::paths::routine_local_toml_path(slug);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let mut body = String::from("[env]\n");
    for (key, value) in entries {
        let _ = writeln!(body, "{key} = {value:?}");
    }
    std::fs::write(&path, body).unwrap();
}

fn test_agent() -> AgentCommand {
    AgentCommand {
        command: "claude".to_string(),
        args: vec![],
        instructions_file: "CLAUDE.md".to_string(),
        setup: None,
    }
}

#[test]
fn build_routine_command_exports_tracked_env_vars() {
    // `routine.toml`'s `[env]` table (loaded into `Routine::env`) is emitted as `export KEY=value`
    // statements, shell-quoted.
    with_home_override(|_| {
        let mut routine = make_routine("Cmd Env Tracked Routine");
        routine.env = HashMap::from([("MY_VAR".to_string(), "hello world".to_string())]);
        let cmd = build_routine_command(&routine, &test_agent(), TriggerSource::Scheduled);
        assert!(
            cmd.contains(&format!("export MY_VAR={}", shell_quote("hello world"))),
            "expected tracked env var exported in: {cmd}"
        );
    });
}

#[test]
fn build_routine_command_env_exports_precede_ts_and_follow_path() {
    // Per issue #408's proposed approach: emitted right after the curated PATH export, before
    // anything else (including the `TS=` timestamp capture) runs.
    with_home_override(|_| {
        let mut routine = make_routine("Cmd Env Order Routine");
        routine.env = HashMap::from([("ORDER_VAR".to_string(), "1".to_string())]);
        let cmd = build_routine_command(&routine, &test_agent(), TriggerSource::Scheduled);
        let path_pos = cmd.find("export PATH=$PATH:").unwrap();
        let env_pos = cmd.find("export ORDER_VAR=").unwrap();
        let ts_pos = cmd.find(r#"TS="$(date +%s)""#).unwrap();
        assert!(
            path_pos < env_pos && env_pos < ts_pos,
            "expected PATH export < env export < TS= in: {cmd}"
        );
    });
}

#[test]
fn build_routine_command_local_toml_overrides_tracked_env_for_the_same_key() {
    // routine.local.toml (untracked, gitignored) layers on top of routine.toml's [env] table; its
    // keys win on conflict (#408 acceptance criteria).
    with_home_override(|_| {
        let mut routine = make_routine("Cmd Env Override Routine");
        let slug = slugify(&routine.title);
        routine.env = HashMap::from([("SHARED_KEY".to_string(), "tracked-value".to_string())]);
        write_local_env(&slug, &[("SHARED_KEY", "secret-value")]);

        let cmd = build_routine_command(&routine, &test_agent(), TriggerSource::Scheduled);
        assert!(
            cmd.contains(&format!(
                "export SHARED_KEY={}",
                shell_quote("secret-value")
            )),
            "expected routine.local.toml's value to win in: {cmd}"
        );
        assert!(
            !cmd.contains("tracked-value"),
            "the overridden tracked value must not appear in: {cmd}"
        );
    });
}

#[test]
fn build_routine_command_merges_local_and_tracked_env_for_distinct_keys() {
    // Distinct keys from both sources are both present — this is a merge, not a replace.
    with_home_override(|_| {
        let mut routine = make_routine("Cmd Env Merge Routine");
        let slug = slugify(&routine.title);
        routine.env = HashMap::from([("TRACKED_ONLY".to_string(), "a".to_string())]);
        write_local_env(&slug, &[("LOCAL_ONLY", "b")]);

        let cmd = build_routine_command(&routine, &test_agent(), TriggerSource::Scheduled);
        assert!(cmd.contains("export TRACKED_ONLY='a'"), "in: {cmd}");
        assert!(cmd.contains("export LOCAL_ONLY='b'"), "in: {cmd}");
    });
}

#[test]
fn build_routine_command_skips_invalid_env_key_from_local_toml() {
    // routine.local.toml is a hand-edited file that never passes through the API's
    // `validate_env` — a malformed key must be dropped (not crash the launch, not break the
    // `;`-joined command) rather than trusted verbatim.
    with_home_override(|_| {
        let mut routine = make_routine("Cmd Env Invalid Key Routine");
        let slug = slugify(&routine.title);
        write_local_env(&slug, &[("not-a-valid-key", "value"), ("VALID_KEY", "ok")]);
        routine.env = HashMap::new();

        let cmd = build_routine_command(&routine, &test_agent(), TriggerSource::Scheduled);
        assert!(
            !cmd.contains("not-a-valid-key"),
            "invalid key must be dropped, not exported, in: {cmd}"
        );
        assert!(
            cmd.contains("export VALID_KEY='ok'"),
            "the valid sibling entry must still be exported in: {cmd}"
        );
    });
}

#[test]
fn build_routine_command_skips_env_value_containing_a_newline() {
    // A newline in a value would split the single-line, `;`-joined launch command into two shell
    // statements — an injection quoting alone can't neutralize. Dropped defensively even though
    // `validate_env` should already have rejected this at create/update time.
    with_home_override(|_| {
        let mut routine = make_routine("Cmd Env Newline Routine");
        routine.env = HashMap::from([("BAD_VAR".to_string(), "line1\nrm -rf /".to_string())]);

        let cmd = build_routine_command(&routine, &test_agent(), TriggerSource::Scheduled);
        assert!(
            !cmd.contains("BAD_VAR"),
            "a newline-carrying value must not be exported at all, in: {cmd}"
        );
    });
}

#[test]
fn build_routine_command_no_env_exports_when_none_set() {
    // No `[env]` table and no routine.local.toml: behaves exactly as before this feature existed.
    with_home_override(|_| {
        let routine = make_routine("Cmd Env Absent Routine");
        let cmd = build_routine_command(&routine, &test_agent(), TriggerSource::Scheduled);
        assert_eq!(
            cmd.matches("export ").count(),
            1,
            "expected only the PATH export with no routine env configured, in: {cmd}"
        );
    });
}

#[test]
fn is_valid_env_key_accepts_shell_identifiers_and_rejects_the_rest() {
    assert!(is_valid_env_key("FOO"));
    assert!(is_valid_env_key("_FOO_BAR_9"));
    assert!(is_valid_env_key("a"));
    assert!(!is_valid_env_key(""));
    assert!(!is_valid_env_key("9FOO"));
    assert!(!is_valid_env_key("FOO-BAR"));
    assert!(!is_valid_env_key("FOO BAR"));
    assert!(!is_valid_env_key("FOO=BAR"));
}
