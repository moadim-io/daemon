---
"moadim": patch
---

refactor(routes): move restart HTTP + MCP endpoints into `routes/restart`

Follows the `routes/health/` / `routes/shutdown/` template (see
`src/routes/CONTRIBUTING.md`): splits the `POST /restart` handler and the MCP
`restart` tool into `src/routes/restart/` — `mod.rs` (wiring), `logic.rs`
(the shared `RestartResponse` type and a `build()` that spawns the detached
restart helper and builds the response), `http.rs`, and `mcp.rs` (declared as
a child module of `routes::mcp` so it keeps access to `MoadimMcp`'s private
state). Both surfaces now call the same `logic::build()` instead of each
spawning the helper and building the response separately.

No behavior change: same response fields, same log messages on each surface.
