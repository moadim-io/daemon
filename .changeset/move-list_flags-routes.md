---
"moadim": patch
---

refactor(routes): move list_flags HTTP + MCP endpoints into `routes/list_flags`

Follows the `routes/health/` / `routes/shutdown/` / `routes/restart/` /
`routes/get_lock_status/` / `routes/list_agents/` / `routes/cleanup_workbenches/` /
`routes/list_routines/` / `routes/get_routine/` / `routes/delete_routine/` /
`routes/create_routine/` / `routes/list_routine_runs/` / `routes/update_routine/` /
`routes/trigger_routine/` / `routes/create_flag/` template (see
`src/routes/CONTRIBUTING.md`): splits the `GET /routines/{id}/flags` handler
(previously `routines::list_flags` in `src/routines/handlers.rs`) and the MCP
`list_flags` tool into `src/routes/list_flags/` — `mod.rs` (wiring), `logic.rs`
(a `build()` that wraps `crate::routines::svc_list_flags`), `http.rs`, and
`mcp.rs` (declared as a child module of `routes::mcp` so it keeps access to
`MoadimMcp`'s private state). Both surfaces now call the same `logic::build()`
instead of each hand-calling `svc_list_flags`.

`resolve_flag` is left as-is in `routines::handlers` / `routes::mcp` for
now — it's a separate MCP tool + REST handler pair sharing the same
`/routines/{id}/flags/{filename}` path family, split out in its own follow-up
PR.

No behavior change: same response (`Vec<Flag>`, 200; 404 when the routine
doesn't exist).
