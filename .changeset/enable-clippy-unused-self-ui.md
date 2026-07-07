---
"moadim": patch
---

chore(lint): enable `clippy::unused_self` in the `ui` crate

The `ui` (Yew/WASM) crate has its own `[lints.clippy]` table with only `all = "deny"` — it does not inherit the workspace root's extended deny-list via `[lints] workspace = true`, so every lint enabled in root `Cargo.toml` (e.g. `unused_self`, already denied since #1067) silently never applied to `ui/src` despite CI's `clippy` job running `--workspace`. Enables it in `ui/Cargo.toml`; the `ui` crate already has zero violations, so no code changes are needed. No behavior change.
