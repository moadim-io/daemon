//! Env-key and env-value validation cases split out of `command_env_tests.rs` (line budget).

use super::*;

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
            cmd.contains(r#"export VALID_KEY="$MOADIM_ROUTINE_ENV__VALID_KEY""#),
            "the valid sibling entry must still be exported in: {cmd}"
        );
        assert_eq!(
            routine_env_carrier_vars(&routine).get("MOADIM_ROUTINE_ENV__VALID_KEY"),
            Some(&"ok".to_string())
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

#[test]
fn env_export_stmts_deliver_tricky_values_byte_exact_through_a_real_shell() {
    // The indirection `export KEY="$carrier"` must hand the agent shell the exact value, with no
    // re-parsing: quotes, `$`, backticks, globs, backslashes, spaces and non-ASCII all survive.
    let values = [
        ("V_SPACES", "  two  spaces  "),
        ("V_QUOTES", r#"it's "quoted""#),
        ("V_DOLLAR", "$HOME and ${PATH}"),
        ("V_BACKTICK", "`id` $(id)"),
        ("V_GLOB_BS", r"* ? [a] back\slash"),
        ("V_UNICODE", "ünïcødé ✓ שלום"),
    ];
    with_home_override(|_| {
        let mut routine = make_routine("Cmd Env Tricky Values Routine");
        routine.env = values
            .iter()
            .map(|(name, value)| ((*name).to_string(), (*value).to_string()))
            .collect();
        let stmts = env_export_stmts(&routine).join("; ");
        for (key, expected) in values {
            let output = std::process::Command::new("/bin/sh")
                .arg("-c")
                .arg(format!(r#"{stmts}; printf '%s' "${key}""#))
                .envs(routine_env_carrier_vars(&routine))
                .output()
                .unwrap();
            assert!(output.status.success(), "sh failed for {key}: {output:?}");
            assert_eq!(String::from_utf8(output.stdout).unwrap(), expected, "value of {key}");
        }
    });
}
