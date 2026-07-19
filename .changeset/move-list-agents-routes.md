---
"moadim": patch
---

refactor(routes): move list_agents HTTP + MCP endpoints into `routes/list_agents`

Follows the `routes/health/` / `routes/shutdown/` / `routes/restart/` /
`routes/get_lock_status/` template (see `src/routes/CONTRIBUTING.md`): splits
the `GET /agents` handler (previously in `src/routines/handlers.rs`) and the
MCP `list_agents` tool into `src/routes/list_agents/` — `mod.rs` (wiring),
`logic.rs` (a `build()` that wraps `crate::routines::available_agents()`),
`http.rs`, and `mcp.rs` (declared as a child module of `routes::mcp` so it
keeps access to `MoadimMcp`'s private state). Both surfaces now call the same
`logic::build()` instead of each calling `available_agents()` separately.

No behavior change: same response (array of available agent registry keys).
