---
"moadim": patch
---

chore(lint): enable clippy::if_not_else in the ui crate

Mirrors the root crate's `if_not_else = "deny"` (see `Cargo.toml`). The `ui` crate has its
own `[lints.clippy]` table and doesn't inherit root's extended deny-list, so this never
applied to `ui/src` despite CI's `clippy` job running `--workspace`. Fixed the 2 violations
this surfaced: `ui/src/header.rs`'s version-title span and `ui/src/overview.rs`'s attention
panel each wrote `if !x.is_empty() { A } else { B }`, rewritten as `if x.is_empty() { B }
else { A }` to drop the double-negation. No behavior change.
