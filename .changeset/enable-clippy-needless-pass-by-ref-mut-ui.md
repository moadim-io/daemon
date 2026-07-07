---
"moadim": patch
---

chore(lint): enable `clippy::needless_pass_by_ref_mut` in the `ui` crate

Mirrors the root crate's existing `needless_pass_by_ref_mut = "deny"` into `ui/Cargo.toml`'s own `[lints.clippy]` table, which doesn't inherit root's extended deny-list. Locks in the crate's existing zero-violation state so a future stale `&mut` parameter fails CI instead of overstating what the function does to its caller. No behavior change.
