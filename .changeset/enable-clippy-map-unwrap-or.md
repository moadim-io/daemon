---
"moadim": patch
---

chore(lint): enable `clippy::map_unwrap_or`

Adds `map_unwrap_or = "deny"` to the workspace root `Cargo.toml`'s `[lints.clippy]` table, rejecting `.map(f).unwrap_or(_)`/`.map(f).unwrap_or_else(_)` in favour of the idiomatic `map_or`/`map_or_else`/`is_ok_and` single-combinator form. Fixes the violations this surfaced across `src/` at the time (`routes/mcp.rs`, `routine_storage.rs`, `routines/cleanup/mod.rs`, `routines/cleanup/session.rs`, `utils/time.rs`). No behavior change. (#524)
