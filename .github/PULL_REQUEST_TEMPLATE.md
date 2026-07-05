<!--
Thanks for contributing to moadim! Keep PRs small and focused — one logical
change per PR. Fill in the sections below; delete any that don't apply.
-->

## What & why

<!-- What does this change do, and what problem does it solve? -->

Fixes #<!-- issue number, if any -->

## Checklist

The pre-push hook and CI enforce these — running them locally first avoids a red PR (see [CONTRIBUTING.md](../CONTRIBUTING.md)):

- [ ] `cargo fmt --check` passes
- [ ] `cargo clippy` is clean
- [ ] `cargo test` passes
- [ ] 100% line coverage holds (`cargo llvm-cov --fail-under-lines 100 --ignore-filename-regex 'src/main\.rs'`)
- [ ] Tests live in `*_tests.rs` sibling files (not inline `#[cfg(test)] mod foo { … }` blocks)
- [ ] `CHANGELOG.md` has an entry under `## [Unreleased]` (required for any `src/` or `ui/` change)
- [ ] Docs updated if behavior, CLI flags, or API shapes changed
