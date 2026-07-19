---
"moadim": patch
---

refactor(routes): move update_routine HTTP + MCP endpoints into `routes/update_routine`

Follows the `routes/health/` / `routes/shutdown/` / `routes/restart/` /
`routes/get_lock_status/` / `routes/list_agents/` / `routes/cleanup_workbenches/` /
`routes/list_routines/` / `routes/get_routine/` / `routes/delete_routine/` /
`routes/create_routine/` / `routes/list_routine_runs/` template (see
`src/routes/CONTRIBUTING.md`): splits the `PATCH /routines/{id}` handler
(previously `routines::update` in `src/routines/handlers.rs`, with
`routines::replace` as its `PUT` alias) and the MCP `update_routine` tool into
`src/routes/update_routine/` — `mod.rs` (wiring), `logic.rs` (a `build()` that
wraps `crate::routines::svc_update`), `http.rs` (keeps both the `PATCH` handler
and the `PUT` alias, still offloading to `spawn_blocking` since `svc_update`
syncs the crontab), and `mcp.rs` (declared as a child module of `routes::mcp`
so it keeps access to `MoadimMcp`'s private state). Both surfaces now call the
same `logic::build()` instead of each hand-calling `svc_update`.

No behavior change: same response (the updated routine record, 400 on invalid
fields, 404 when missing).
