---
"moadim": patch
---

refactor(routes): move list_routine_runs HTTP + MCP endpoints into `routes/list_routine_runs`

Follows the `routes/health/` / `routes/shutdown/` / `routes/restart/` /
`routes/get_lock_status/` / `routes/list_agents/` / `routes/cleanup_workbenches/` /
`routes/list_routines/` / `routes/get_routine/` / `routes/delete_routine/` /
`routes/create_routine/` template (see `src/routes/CONTRIBUTING.md`): splits the
`GET /routines/{id}/runs` handler (previously `routines::get_runs` in
`src/routines/handlers.rs`) and the MCP `list_routine_runs` tool into
`src/routes/list_routine_runs/` — `mod.rs` (wiring), `logic.rs` (a `build()`
that wraps `crate::routines::svc_list_runs`), `http.rs`, and `mcp.rs`
(declared as a child module of `routes::mcp` so it keeps access to
`MoadimMcp`'s private state). Both surfaces now call the same `logic::build()`
instead of each hand-calling `svc_list_runs`.

No behavior change: same response (a routine's runs, newest first, 404 when
the routine is missing).
