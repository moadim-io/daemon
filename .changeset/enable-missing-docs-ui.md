---
"moadim": patch
---

chore(lint): enable `missing_docs` in the `ui` crate

The `ui` crate has its own `[lints]` table (no `workspace = true` inheritance), so root's
`missing_docs = "deny"` (in force since the project's early `[lints.rust]` table) never applied to
`ui/src` despite CI's `clippy`/`doc` jobs running `--workspace`. Enabling it surfaced 34 undocumented
public items in `main.rs`: the crate root doc, the `Route` enum and its variants, `ShellState` and
its fields, `ShellAction` and its variants/fields, and the `App`/`Nav` function components. Added
doc comments for each, and split `ShellState`/`ShellAction`/their `Reducible` impl out into a new
`shell_state.rs` module so `main.rs` stays under the workspace's 500-line-per-file convention. No
behavior change.
