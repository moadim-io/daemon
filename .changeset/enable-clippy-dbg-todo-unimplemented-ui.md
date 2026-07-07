---
"moadim": patch
---

chore(lint): enable `clippy::dbg_macro`, `clippy::todo`, and `clippy::unimplemented` in the `ui` crate

The `ui` (Yew/WASM) crate has its own `[lints.clippy]` table with only `all = "deny"` — it does not inherit the workspace root's extended deny-list via `[lints] workspace = true`, so every lint enabled in root `Cargo.toml` (e.g. `dbg_macro`, `todo`, `unimplemented`) silently never applied to `ui/src` despite CI's `clippy` job running `--workspace`. A stray `dbg!()`, `todo!()`, or `unimplemented!()` left in the UI crate would ship straight into the release build and panic the running Yew app on that code path. Enables all three in `ui/Cargo.toml`; the `ui` crate already has zero violations, so no code changes are needed. No behavior change.
