---
"moadim": patch
---

chore(lint): enable `clippy::unseparated_literal_suffix` workspace-wide

Denies a numeric literal whose type suffix isn't underscore-separated from its digits (e.g.
`500u64` instead of `500_u64`), matching the existing `unreadable_literal` convention for
large integer literals. Enabled in both the root `Cargo.toml` and `ui/Cargo.toml` (the `ui`
crate has its own `[lints.clippy]` table and doesn't inherit root's deny-list). Fixed the 48
violations this surfaced (27 root, 21 `ui`) via `cargo clippy --fix` — all mechanical
suffix-underscore insertions, no behavior change.
