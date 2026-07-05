---
"moadim": patch
---

chore: fix the three files that had grown past the 700-line pre-push gate

`src/routines/service.rs` (701 lines), `src/routines/cleanup/cleanup_tests.rs` (740 lines),
and `src/service/mod_tests.rs` (816 lines) had all grown past the repo's 700-line pre-push
gate (`.githooks/pre-push`'s `linecheck` step), which currently fails on plain `main` for
anyone who has the git hooks installed per CONTRIBUTING.md.

- Moves `service.rs`'s dozen field-validation helpers (`reject_blank`, `validate_title`,
  `validate_repositories`, `validate_agent`, and friends) into a new
  `src/routines/service_validate.rs`, matching the file's existing convention of
  `#[path = "..."]`-declared sibling modules. No behavior changed; `service.rs` drops to
  457 lines.
- Moves `cleanup_tests.rs`'s tmux/session-probe tests (`tmux_kill_session_is_best_effort_*`,
  `tmux_session_alive_*`, `tmux_session_prefix_alive_*`, and friends) into a new
  `src/routines/cleanup/cleanup_tmux_tests.rs`, matching the existing one-file-per-concern
  convention already used by `cleanup_claude_json_tests.rs` and `cleanup_freed_bytes_tests.rs`.
  `cleanup_tests.rs` drops to 543 lines.
- Moves `mod_tests.rs`'s Linux-only systemd-unit and loginctl/linger tests into a new
  `src/service/mod_linux_tests.rs`, mirroring the macOS/Linux backend split already present
  in `service/macos.rs` and `service/linux.rs`. `mod_tests.rs` drops to 382 lines.

No test bodies changed; `cargo test` and `cargo llvm-cov --fail-under-lines 100` still pass,
and the full pre-push gate (`SKIP_CHANGELOG=1 sh .githooks/pre-push`) now exits 0 on `main`.
