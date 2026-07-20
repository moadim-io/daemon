---
"moadim": patch
---

refactor(routes): move lock_routines HTTP + MCP endpoints into `routes/lock_routines`

Follows the `routes/health/` / `routes/shutdown/` / `routes/restart/` /
`routes/get_lock_status/` / `routes/list_agents/` / `routes/cleanup_workbenches/` /
`routes/list_routines/` / `routes/get_routine/` / `routes/delete_routine/` /
`routes/create_routine/` / `routes/list_routine_runs/` / `routes/update_routine/` /
`routes/trigger_routine/` / `routes/create_flag/` / `routes/list_flags/` /
`routes/resolve_flag/` template (see `src/routes/CONTRIBUTING.md`): splits the
`POST /routines/lock` handler (previously `routines::lock` in
`src/routines/handlers.rs`, named `lock`) and the MCP `lock_routines` tool
into `src/routes/lock_routines/` — `mod.rs` (wiring), `logic.rs` (a `build()`
that validates the scope, creates the lock sentinel, and syncs the crontab),
`http.rs` (renamed handler `lock_routines`, still offloading to
`spawn_blocking` since the crontab sync shells out to `crontab`(1)), and
`mcp.rs` (declared as a child module of `routes::mcp` so it keeps access to
`MoadimMcp`'s private state). Both surfaces now call the same
`logic::build()` instead of each hand-validating the scope and syncing the
crontab separately.

`unlock`/`unlock_routines` are unchanged and still live in
`src/routines/handlers.rs` / `routes/mcp.rs` — a future PR will split those
out the same way.

No behavior change: same response (the current lock status), 400 on an
unknown scope, 500 on IO failure.
