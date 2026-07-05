---
"moadim": patch
---

chore(cli): split the bind-override tests out of `cli_tests.rs`

`src/cli_tests.rs` had grown to 705 lines, past the repo's 700-line pre-push
gate (`.githooks/pre-push`'s `linecheck` step), which currently fails on
plain `main` for anyone who has the git hooks installed per CONTRIBUTING.md.

Moves the 7 `BIND_ADDR_ENV`-override tests (`bind_addr_uses_default_when_unset`,
`bind_addr_honors_override`, `status_json_address_reflects_bind_override`, and
friends) into a new `src/cli_bind_override_tests.rs`, with its own `EnvGuard`
copy, matching the existing one-helper-copy-per-test-file convention already
used by `cli_json_tests.rs` and `cli_spawn_tests.rs`. No test bodies changed;
`cli_tests.rs` drops to 617 lines. `cargo test` still passes the same 101
`cli::*` tests.
