---
"moadim": patch
---

refactor(routes): move unlock_routines HTTP + MCP endpoints into `routes/unlock_routines`

Follows the `routes/health/` / `routes/shutdown/` / `routes/restart/` /
`routes/get_lock_status/` / `routes/list_agents/` / `routes/cleanup_workbenches/` /
`routes/list_routines/` / `routes/get_routine/` / `routes/delete_routine/` /
`routes/create_routine/` / `routes/list_routine_runs/` / `routes/update_routine/` /
`routes/trigger_routine/` / `routes/create_flag/` / `routes/list_flags/` /
`routes/resolve_flag/` / `routes/lock_routines/` template (see
`src/routes/CONTRIBUTING.md`): splits the `DELETE /routines/lock` handler
(previously `routines::unlock` in `src/routines/handlers.rs`) and the MCP
`unlock_routines` tool into `src/routes/unlock_routines/` — `mod.rs`
(wiring), `logic.rs` (a `build()` that validates the scope — now including
`"all"` in the single shared parser instead of special-casing it at each
call site — removes the matching lock sentinel(s), and syncs the crontab),
`http.rs` (renamed handler `unlock_routines`, still offloading to
`spawn_blocking` since the crontab sync shells out to `crontab`(1)), and
`mcp.rs` (declared as a child module of `routes::mcp` so it keeps access to
`MoadimMcp`'s private state). Both surfaces now call the same
`logic::build()` instead of each hand-validating the scope and syncing the
crontab separately.

`snooze_routine` and `set_power_saving` have no REST counterpart, so they
stay as-is in `routes/mcp.rs`.

No behavior change: same response (the current lock status), 400 on an
unknown scope, 500 on IO failure.
