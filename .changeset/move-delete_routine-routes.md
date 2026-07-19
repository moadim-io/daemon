---
"moadim": patch
---

refactor(routes): move delete_routine HTTP + MCP endpoints into `routes/delete_routine`

Follows the `routes/health/` / `routes/shutdown/` / `routes/restart/` /
`routes/get_lock_status/` / `routes/list_agents/` / `routes/cleanup_workbenches/` /
`routes/list_routines/` / `routes/get_routine/` template (see
`src/routes/CONTRIBUTING.md`): splits the `DELETE /routines/{id}` handler
(previously `routines::delete` in `src/routines/handlers.rs`) and the MCP
`delete_routine` tool into `src/routes/delete_routine/` — `mod.rs` (wiring),
`logic.rs` (a `build()` that wraps `crate::routines::svc_delete`), `http.rs`
(keeps the `spawn_blocking` offload since `svc_delete` syncs the crontab), and
`mcp.rs` (declared as a child module of `routes::mcp` so it keeps access to
`MoadimMcp`'s private state). Both surfaces now call the same `logic::build()`
instead of each hand-calling `svc_delete`.

No behavior change: same response (the deleted routine record, 404 when missing).
