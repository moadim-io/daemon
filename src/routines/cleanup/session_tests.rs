#![allow(
    clippy::missing_docs_in_private_items,
    reason = "test helpers and fixtures do not need doc comments"
)]

use super::*;

#[test]
fn is_fire_of_prefix_matches_same_routine_rid() {
    // Real shape emitted by `SESS="moadim-$SLUG-$RID"` where `RID="${TS}_$$"`.
    assert!(is_fire_of_prefix(
        "moadim-deploy-1730000000_4821",
        "moadim-deploy-"
    ));
}

#[test]
fn is_fire_of_prefix_rejects_a_different_routine_whose_slug_is_a_prefix() {
    // Regression for the overlap-guard false positive: slug `deploy` is a plain string-prefix of
    // slug `deploy-staging`, so `"moadim-deploy-staging-<rid>".starts_with("moadim-deploy-")` used
    // to read as "deploy's own fire is still alive" and silently skip deploy's launch.
    assert!(!is_fire_of_prefix(
        "moadim-deploy-staging-1730000000_4821",
        "moadim-deploy-"
    ));
}

#[test]
fn is_fire_of_prefix_rejects_non_rid_suffix() {
    assert!(!is_fire_of_prefix(
        "moadim-deploy-not-a-rid",
        "moadim-deploy-"
    ));
}

#[test]
fn is_fire_of_prefix_rejects_missing_prefix() {
    assert!(!is_fire_of_prefix("other-session-1_2", "moadim-deploy-"));
}
