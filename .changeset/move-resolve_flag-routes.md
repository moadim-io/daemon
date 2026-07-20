---
"moadim": patch
---

refactor(routes): move resolve_flag HTTP + MCP endpoints into `routes/resolve_flag`

Follows the `routes/health/` / `routes/shutdown/` / `routes/restart/` /
`routes/get_lock_status/` / `routes/list_agents/` / `routes/cleanup_workbenches/` /
`routes/list_routines/` / `routes/get_routine/` / `routes/delete_routine/` /
`routes/create_routine/` / `routes/list_routine_runs/` / `routes/update_routine/` /
`routes/trigger_routine/` / `routes/create_flag/` / `routes/list_flags/` template
(see `src/routes/CONTRIBUTING.md`): splits the `DELETE
/routines/{id}/flags/{filename}` handler (previously `routines::resolve_flag` in
`src/routines/handlers.rs`) and the MCP `resolve_flag` tool into
`src/routes/resolve_flag/` — `mod.rs` (wiring), `logic.rs` (a `build()` that wraps
`crate::routines::svc_resolve_flag`), `http.rs`, and `mcp.rs` (declared as a child
module of `routes::mcp` so it keeps access to `MoadimMcp`'s private state). Both
surfaces now call the same `logic::build()` instead of each hand-calling
`svc_resolve_flag`.

No behavior change: same response (204 on success, error on a missing routine or
flag).
