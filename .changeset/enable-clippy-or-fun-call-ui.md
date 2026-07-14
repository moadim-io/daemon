---
"moadim": patch
---

chore(lint): enable clippy::or_fun_call in the ui crate

Mirrors the root crate's `or_fun_call = "deny"` (see `Cargo.toml`). The `ui` crate has its own
`[lints.clippy]` table and doesn't inherit root's extended deny-list, so this never applied to
`ui/src` despite CI's `clippy` job running `--workspace`. The `ui` crate is already clean under
it (zero violations), so `deny` locks that in. No behavior change.
