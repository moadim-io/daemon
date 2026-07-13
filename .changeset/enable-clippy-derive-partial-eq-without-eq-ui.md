---
"moadim": patch
---

chore(lint): enable clippy::derive_partial_eq_without_eq in the ui crate

Mirrors the root crate's `derive_partial_eq_without_eq = "deny"` (see `Cargo.toml`). The
`ui` crate has its own `[lints.clippy]` table and doesn't inherit root's extended deny-list,
so this never applied to `ui/src` despite CI's `clippy` job running `--workspace`. Fixed the
15 violations this surfaced by adding `Eq` alongside `PartialEq` on the affected structs and
enums, all of which are already field-for-field `Eq`-safe (no float fields).
