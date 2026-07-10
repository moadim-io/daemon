---
"moadim": patch
---

chore(lint): enable `clippy::manual_string_new` in the `ui` crate

Adds `manual_string_new = "deny"` to `ui/Cargo.toml`'s `[lints.clippy]` table, matching the
lint already denied workspace-root-side in `Cargo.toml` — the `ui` crate has its own `[lints]`
table (no `workspace = true` inheritance) so it silently escaped this despite CI's `clippy` job
running `--workspace`. The `ui` crate is already clean under this lint, so no code changes are
needed; `deny` just locks that state in. No behavior change.
