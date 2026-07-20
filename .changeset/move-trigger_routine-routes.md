---
"moadim": patch
---

refactor(routes): move trigger_routine HTTP + MCP endpoints into `routes/trigger_routine`

Follows the `routes/health/` / `routes/shutdown/` / `routes/restart/` /
`routes/get_lock_status/` / `routes/list_agents/` / `routes/cleanup_workbenches/` /
`routes/list_routines/` / `routes/get_routine/` / `routes/delete_routine/` /
`routes/create_routine/` / `routes/list_routine_runs/` / `routes/update_routine/`
template (see `src/routes/CONTRIBUTING.md`): splits the `POST
/routines/{id}/trigger` handler (previously `routines::trigger` in
`src/routines/handlers.rs`) and the MCP `trigger_routine` tool into
`src/routes/trigger_routine/` — `mod.rs` (wiring), `logic.rs` (a `build()`
that wraps `crate::routines::svc_trigger`), `http.rs` (still offloading to
`spawn_blocking` since `svc_trigger` shells out to `tmux`(1) and does blocking
fs I/O), and `mcp.rs` (declared as a child module of `routes::mcp` so it keeps
access to `MoadimMcp`'s private state). Both surfaces now call the same
`logic::build()` instead of each hand-calling `svc_trigger`.

No behavior change: same response (the triggered routine record, 423 when
disabled or in power-saving mode, 404 when missing).
