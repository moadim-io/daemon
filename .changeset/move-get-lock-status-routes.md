---
"moadim": patch
---

refactor(routes): move get_lock_status HTTP + MCP endpoints into `routes/get_lock_status`

Follows the `routes/health/` / `routes/shutdown/` / `routes/restart/` template
(see `src/routes/CONTRIBUTING.md`): splits the `GET /routines/lock` handler
(previously in `src/routines/handlers.rs`) and the MCP `get_lock_status` tool
into `src/routes/get_lock_status/` — `mod.rs` (wiring), `logic.rs` (a
`build()` that wraps `crate::global_lock::lock_status()`), `http.rs`, and
`mcp.rs` (declared as a child module of `routes::mcp` so it keeps access to
`MoadimMcp`'s private state). Both surfaces now call the same `logic::build()`
instead of each calling `crate::global_lock::lock_status()` separately.

No behavior change: same response fields (`shared`, `local`, `locked`).
