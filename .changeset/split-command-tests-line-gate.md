---
"moadim": patch
---

chore(routines): split `command_tests.rs`'s binary-resolution tests into a sibling file

`src/routines/command_tests.rs` had grown to 713 lines, past the repo's 700-line pre-push
gate (`.githooks/pre-push`'s `linecheck` step), which currently fails on plain `main` for
anyone who has the git hooks installed per CONTRIBUTING.md.

Moves the 18 `tmux`/agent-binary-resolution tests (`tmux_available_*`,
`agent_command_available_*`, `resolve_tmux_bin_*`, `bin_dir_returns_none_when_path_unset`,
`tmux_fallback_dirs_are_anchored_under_home`) into a new
`src/routines/command_bin_resolution_tests.rs`, matching the existing one-helper-copy-per-
test-file convention already used by `command_run_id_tests.rs` and friends. No test bodies
changed; `command_tests.rs` drops to 462 lines. `cargo test` still passes with the same 886
tests, and `cargo llvm-cov` shows `routines/command.rs` unchanged at 100% line coverage.
