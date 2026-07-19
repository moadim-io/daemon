---
"moadim": patch
---

refactor(routes): move cleanup_workbenches HTTP + MCP endpoints into `routes/cleanup_workbenches`

Follows the `routes/health/` / `routes/shutdown/` / `routes/restart/` /
`routes/get_lock_status/` / `routes/list_agents/` template (see
`src/routes/CONTRIBUTING.md`): splits the `POST /routines/cleanup` handler
(previously `cleanup` in `src/routines/handlers.rs`) and the MCP
`cleanup_workbenches` tool into `src/routes/cleanup_workbenches/` — `mod.rs`
(wiring), `logic.rs` (a `build()` that wraps `crate::routines::svc_cleanup()`
and re-exports `CleanupResponse`), `http.rs`, and `mcp.rs` (declared as a
child module of `routes::mcp` so it keeps access to `MoadimMcp`'s private
state). Both surfaces now call the same `logic::build()` instead of each
calling `svc_cleanup()` separately.

No behavior change: same response shape (`removed`, `freed_bytes`), same
`spawn_blocking` wrapping around the blocking fs/tmux sweep on the HTTP side.
