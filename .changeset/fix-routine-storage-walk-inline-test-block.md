---
"moadim": patch
---

chore: move `routine_storage_walk`'s tests out of an inline `#[cfg(test)]` block

`src/routine_storage_walk.rs` held its unit tests in an inline `#[cfg(test)] mod tests { ... }`
block, which the repo's own convention (see CONTRIBUTING.md) and `.githooks/pre-push`'s
test-file-convention check explicitly forbid in favor of `*_tests.rs` sibling files. Nothing in
CI mirrors that specific check (unlike fmt/clippy/coverage/linecheck, which all have a CI job),
so it silently broke `main` for any contributor running the actual local pre-push hook — running
`sh .githooks/pre-push` on HEAD failed at the very first gate with `FAIL:
src/routine_storage_walk.rs: inline test block found (use *_tests.rs instead)`.

Moves the two tests into `src/routine_storage_walk_tests.rs`, matching the `#[path = "..."] mod
..._tests;` pattern already used by every other `_tests.rs` file in the crate. No behavior
change — the tests are unmodified, just relocated so the hook (and any future CI job that mirrors
it) passes again.
