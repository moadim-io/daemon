---
"moadim": patch
---

### Changed

chore(lint): enable `clippy::match_same_arms` in the `ui` crate

Adds `match_same_arms = "deny"` to `ui/Cargo.toml`'s `[lints.clippy]` table, matching the lint
already denied workspace-root-side in `Cargo.toml` — the `ui` crate has its own `[lints]` table (no
`workspace = true` inheritance) so it silently escaped this despite CI's `clippy` job running
`--workspace`. Fixes the 5 violations this surfaced in `routines/filter.rs`: each facet's explicit
`Facet::All`/`Facet::Any` match arm did nothing that the trailing wildcard arm didn't already do, so
they're removed as dead code. No behavior change.
