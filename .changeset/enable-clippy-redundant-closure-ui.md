---
"moadim": patch
---

chore(lint): enable `clippy::redundant_closure_for_method_calls` in the `ui` crate

The `ui` (Yew/WASM) crate has its own `[lints.clippy]` table with only `all = "deny"` — it does not inherit the workspace root's extended deny-list via `[lints] workspace = true`, so every lint enabled in root `Cargo.toml` (e.g. `redundant_closure_for_method_calls`, already denied since #549) silently never applied to `ui/src` despite CI's `clippy` job running `--workspace`. Enables it in `ui/Cargo.toml` and fixes the 3 violations it surfaces. No behavior change.
