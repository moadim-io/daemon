---
"moadim": patch
---

Move the `moadim` CLI's parsing/lifecycle files (`cli.rs`, `cli_query.rs`, `cli_system.rs`, `cli_restart.rs`, and their `*_tests.rs` siblings — 14 files total) into a `src/cli/` folder, per the TODO.md request to colocate all CLI-command files instead of leaving them as flat, prefix-named siblings in `src/`. Pure file move: module paths, `#[path = ...]` attributes, and one `include_str!("../README.md")` → `("../../README.md")` were updated to match the new depth; no behavior change.
