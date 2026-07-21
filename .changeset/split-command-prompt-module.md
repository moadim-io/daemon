---
"moadim": patch
---

refactor(routines): split prompt composition out of `command.rs` into `command_prompt.rs`

`src/routines/command.rs` had grown to exactly 500 lines — the ceiling
`linecheck` (the `.githooks/pre-push` line-count gate) enforces — so the very
next change to that file, however small, would have broken CI immediately.

Follows the same pattern this file already uses twice
(`command_path_resolution.rs`, `command_system_prompt.rs`): moves
`compose_prompt`, `substitute`, `placeholder_tokens`,
`validate_placeholders`, `MAX_INLINE_PROMPT_BYTES`, and
`inline_prompt_overflow` — the prompt-composition and
`{placeholder}`-substitution/validation logic — into a new
`src/routines/command_prompt.rs`, re-exported from `command.rs` via
`pub(crate) use command_prompt::*;` so every existing
`crate::routines::command::...` import path is unchanged. `command.rs` drops
to 332 lines.

No behavior change and no new tests needed: existing tests
(`command_tests.rs`, `command_placeholder_tests.rs`) keep passing unmodified
against the re-exported items. `cargo fmt`, `cargo clippy --all-targets`,
`cargo test`, and `cargo llvm-cov --fail-under-lines 100` all pass, and line
coverage stays at 100%.
