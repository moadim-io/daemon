---
"moadim": patch
---

### Changed

chore(lint): enable `clippy::map_unwrap_or` in the `ui` crate

Adds `map_unwrap_or = "deny"` to `ui/Cargo.toml`'s `[lints.clippy]` table, matching the lint already
denied workspace-root-side in `Cargo.toml` — the `ui` crate has its own `[lints]` table (no
`workspace = true` inheritance) so it silently escaped this despite CI's `clippy` job running
`--workspace`. Fixes the 7 violations this surfaced across `log_viewer.rs`,
`overview_recent_runs.rs`, `routines/form.rs`, `routines/history.rs`, and `routines/hooks.rs`,
rewriting each `.map(f).unwrap_or(_)`/`.map(f).unwrap_or_else(_)` into the idiomatic
`map_or`/`map_or_else`/`is_some_and` single-combinator form. No behavior change.
