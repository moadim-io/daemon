---
"moadim": patch
---

chore(lint): enable clippy::manual_let_else in the ui crate

Mirrors the root crate's `manual_let_else = "deny"` (see `Cargo.toml`). The `ui` crate has
its own `[lints.clippy]` table and doesn't inherit root's extended deny-list, so this never
applied to `ui/src` despite CI's `clippy` job running `--workspace`. Fixed the 1 violation
this surfaced in `ui/src/routines/state.rs::sort_routines`, rewriting a `match` whose only
non-binding arm returned early into `let Some(col) = col else { return routines };`. No
behavior change.
