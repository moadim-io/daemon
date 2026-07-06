#![allow(
    clippy::missing_docs_in_private_items,
    reason = "test helpers and fixtures do not need doc comments"
)]

use super::*;

#[test]
fn placeholder_tokens_extracts_only_placeholder_shaped_braces() {
    // A leading `{` (i == 0), a known token mid-string, and a typo are all captured...
    assert_eq!(
        placeholder_tokens("{prompt} run {prompt_fil}"),
        vec!["{prompt}".to_string(), "{prompt_fil}".to_string()]
    );
    // ...while shell `${VAR}`, empty `{}`, digit-led `{0}`, uppercase `{Ab}`, a token with a
    // non-identifier char `{a-b}`, and an unclosed `{abc` are all ignored.
    assert!(placeholder_tokens("${HOME} {} {0} {Ab} {a-b} {abc").is_empty());
}

#[test]
fn validate_placeholders_accepts_prompt_and_prompt_file() {
    assert!(validate_placeholders(&["--task".into(), "{prompt}".into()]).is_ok());
    assert!(validate_placeholders(&["{prompt_file}".into(), "{workbench}".into()]).is_ok());
}

#[test]
fn validate_placeholders_rejects_unknown_token() {
    let err = validate_placeholders(&["{prompt_fil}".into()]).unwrap_err();
    assert!(err.contains("unknown placeholder {prompt_fil}"), "{err}");
}

#[test]
fn validate_placeholders_rejects_missing_prompt_placeholder() {
    // Only `{workbench}` (or no placeholder at all) delivers no prompt.
    let err = validate_placeholders(&["{workbench}".into()]).unwrap_err();
    assert!(err.contains("must include a prompt placeholder"), "{err}");
    assert!(validate_placeholders(&[]).is_err());
}
