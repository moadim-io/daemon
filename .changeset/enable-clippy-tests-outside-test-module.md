---
"moadim": patch
---

chore(lint): enable `clippy::tests_outside_test_module` workspace-wide

Rejects a `#[test]` function declared outside a `#[cfg(test)]` module. This formalizes, at the
AST level, this repo's existing `*_tests.rs` convention: `.githooks/pre-push` step 1 already
greps for tests living outside a `#[cfg(test)] mod foo_tests;` sibling, but that check is
text-pattern-based and only runs locally, pre-push. Enabling this lint means `cargo clippy`
(CI's `lint.yml` job) enforces the same invariant from the compiler's own AST, so it can't be
skipped by a contributor who bypasses git hooks.

Enabled in both the root `Cargo.toml` and `ui/Cargo.toml` (the `ui` crate has its own
`[lints.clippy]` table and doesn't inherit root's deny-list), mirroring the parity pattern
already used for `unreachable`, `redundant_type_annotations`, and others. The workspace
(`src/` and `ui/src`) was already clean, so `deny` locks that in with no behavior change.
