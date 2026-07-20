---
"moadim": patch
---

refactor(routes): move create_flag HTTP + MCP endpoints into `routes/create_flag`

Follows the `routes/health/` / `routes/shutdown/` / `routes/restart/` /
`routes/get_lock_status/` / `routes/list_agents/` / `routes/cleanup_workbenches/` /
`routes/list_routines/` / `routes/get_routine/` / `routes/delete_routine/` /
`routes/create_routine/` / `routes/list_routine_runs/` / `routes/update_routine/` /
`routes/trigger_routine/` template (see `src/routes/CONTRIBUTING.md`): splits the
`POST /routines/{id}/flags` handler (previously `routines::create_flag` in
`src/routines/handlers.rs`) and the MCP `create_flag` tool into
`src/routes/create_flag/` — `mod.rs` (wiring), `logic.rs` (a `build()` that wraps
`crate::routines::svc_create_flag`, plus the `CreateFlagRequest` request body,
moved out of `routines::handlers`), `http.rs`, and `mcp.rs` (declared as a child
module of `routes::mcp` so it keeps access to `MoadimMcp`'s private state). Both
surfaces now call the same `logic::build()` instead of each hand-calling
`svc_create_flag`.

`list_flags` and `resolve_flag` are left as-is in `routines::handlers` /
`routes::mcp` for now — they're separate MCP tool + REST handler pairs sharing
the same `/routines/{id}/flags` path family, split out in their own follow-up PRs.

No behavior change: same response (the created `Flag`, 201; 400 on an invalid
`type`/`description`/`scope`; 404 when the routine doesn't exist).
