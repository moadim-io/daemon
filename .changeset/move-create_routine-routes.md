---
"moadim": patch
---

refactor(routes): move create_routine HTTP + MCP endpoints into `routes/create_routine`

Follows the `routes/health/` / `routes/shutdown/` / `routes/restart/` /
`routes/get_lock_status/` / `routes/list_agents/` / `routes/cleanup_workbenches/` /
`routes/list_routines/` / `routes/get_routine/` / `routes/delete_routine/` template
(see `src/routes/CONTRIBUTING.md`): splits the `POST /routines` handler
(previously `routines::create` in `src/routines/handlers.rs`) and the MCP
`create_routine` tool into `src/routes/create_routine/` — `mod.rs` (wiring),
`logic.rs` (a `build()` that wraps `crate::routines::svc_create`), `http.rs`
(keeps the `spawn_blocking` offload since `svc_create` syncs the crontab), and
`mcp.rs` (declared as a child module of `routes::mcp` so it keeps access to
`MoadimMcp`'s private state). Both surfaces now call the same `logic::build()`
instead of each hand-calling `svc_create`.

No behavior change: same response (the created routine record, 400 on an
invalid cron expression).
