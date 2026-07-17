#![allow(
    clippy::missing_docs_in_private_items,
    reason = "test helpers and fixtures do not need doc comments"
)]

use super::*;

/// Verbs the generated completion script must mention, mirroring the acceptance criteria in
/// issue #307: every lifecycle subcommand plus the `--json`/`--quiet` flags shared across them.
const EXPECTED_VERBS: &[&str] = &[
    "restart",
    "stop",
    "status",
    "cleanup",
    "trigger",
    "logs",
    "install",
    "uninstall",
    "machine",
    "help",
    "version",
    "completions",
];

#[test]
fn parse_recognizes_the_completions_command() {
    assert_eq!(
        parse(vec!["completions".to_string(), "zsh".to_string()]),
        Command::Completions(Some("zsh".to_string()))
    );
}

#[test]
fn parse_without_a_shell_carries_no_shell() {
    assert_eq!(
        parse(vec!["completions".to_string()]),
        Command::Completions(None)
    );
}

#[test]
fn bash_zsh_and_fish_each_emit_a_non_empty_script() {
    for shell in ["bash", "zsh", "fish", "powershell", "elvish"] {
        let mut out = Vec::new();
        write_completions(shell, &mut out)
            .unwrap_or_else(|()| panic!("{shell} should be a supported shell"));
        assert!(
            !out.is_empty(),
            "{shell} completion script should not be empty"
        );
    }
}

#[test]
fn generated_script_covers_every_lifecycle_verb() {
    let mut out = Vec::new();
    write_completions("zsh", &mut out).expect("zsh is supported");
    let script = String::from_utf8(out).expect("clap_complete output is valid UTF-8");
    for verb in EXPECTED_VERBS {
        assert!(
            script.contains(verb),
            "zsh completion script is missing the `{verb}` subcommand \
             (keep `build_cli` in sync with `Command`)"
        );
    }
    for flag in ["json", "quiet"] {
        assert!(
            script.contains(flag),
            "zsh completion script is missing the `--{flag}` flag"
        );
    }
}

#[test]
fn unknown_shell_is_rejected() {
    let mut out = Vec::new();
    assert_eq!(write_completions("powersh", &mut out), Err(()));
    assert!(out.is_empty());
}

#[test]
fn completions_returns_success_for_a_supported_shell() {
    assert_eq!(completions(Some("bash")), 0);
}

#[test]
fn completions_returns_usage_error_for_an_unsupported_shell() {
    assert_eq!(completions(Some("nope")), EXIT_USAGE);
}

#[test]
fn completions_returns_usage_error_when_no_shell_given() {
    assert_eq!(completions(None), EXIT_USAGE);
}

#[test]
fn build_cli_has_a_completions_subcommand() {
    let cli = build_cli();
    assert!(cli
        .get_subcommands()
        .any(|sub| sub.get_name() == "completions"));
}
