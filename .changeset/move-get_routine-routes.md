---
"moadim": patch
---

refactor(routes): move get_routine HTTP + MCP endpoints into `routes/get_routine`

Follows the `routes/health/` / `routes/shutdown/` / `routes/restart/` /
`routes/get_lock_status/` / `routes/list_agents/` / `routes/cleanup_workbenches/` /
`routes/list_routines/` template (see `src/routes/CONTRIBUTING.md`): splits the
`GET /routines/{id}` handler (previously `routines::get` in
`src/routines/handlers.rs`) and the MCP `get_routine` tool into
`src/routes/get_routine/` — `mod.rs` (wiring), `logic.rs` (a `build()` that
wraps `crate::routines::svc_get`), `http.rs`, and `mcp.rs` (declared as a child
module of `routes::mcp` so it keeps access to `MoadimMcp`'s private state).
Both surfaces now call the same `logic::build()` instead of each hand-calling
`svc_get`.

No behavior change: same response (a single routine by UUID, 404 when missing).
