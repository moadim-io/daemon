---
"moadim": patch
---

chore(lint): enable `clippy::string_lit_as_bytes` in the `ui` crate. Mirrors the same lint already
enabled root-side (#1202) — the `ui` crate has its own `[lints.clippy]` table and doesn't inherit
root's extended deny-list, so this never applied to `ui/src` despite CI's `clippy` job running
`--workspace`. The `ui` crate is already clean (0 violations), so `deny` locks that in. No behavior
change.
