---
"moadim": patch
---

refactor(routes): move list_routines HTTP + MCP endpoints into `routes/list_routines`

Follows the `routes/health/` / `routes/shutdown/` / `routes/restart/` /
`routes/get_lock_status/` / `routes/list_agents/` / `routes/cleanup_workbenches/`
template (see `src/routes/CONTRIBUTING.md`): splits the `GET /routines` handler
(previously `routines::list` in `src/routines/handlers.rs`) and the MCP
`list_routines` tool into `src/routes/list_routines/` — `mod.rs` (wiring),
`logic.rs` (a `build()` that wraps `crate::routines::svc_list`), `http.rs`,
and `mcp.rs` (declared as a child module of `routes::mcp` so it keeps access
to `MoadimMcp`'s private state). Both surfaces now call the same
`logic::build()` instead of each hand-assembling the same call to `svc_list`.

No behavior change: same response (routine list, `local_only`/`include_prompts`
still respected on both surfaces).
