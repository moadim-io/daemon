---
"moadim": patch
---

chore(lint): enable `clippy::unnested_or_patterns` in the `ui` crate

Mirrors the root crate's `unnested_or_patterns = "deny"` (root `Cargo.toml`) — the `ui` crate has
its own `[lints.clippy]` table and doesn't inherit root's extended deny-list, so this never
applied to `ui/src` despite CI's `clippy` job running `--workspace`. The `ui` crate was already
clean, so this surfaced 0 violations. No behavior change.
