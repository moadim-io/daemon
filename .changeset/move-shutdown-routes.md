---
"moadim": patch
---

refactor(routes): move shutdown HTTP + MCP endpoints into `routes/shutdown`

Follows the `routes/health/` template (see `src/routes/CONTRIBUTING.md`):
splits the `POST /shutdown` handler and the MCP `shutdown` tool into
`src/routes/shutdown/` — `mod.rs` (wiring), `logic.rs` (the shared
`ShutdownResponse` type and a `build()` that fires the signal and builds the
response), `http.rs`, and `mcp.rs` (declared as a child module of
`routes::mcp` so it keeps access to `MoadimMcp`'s private state). Both
surfaces now call the same `logic::build()` instead of each notifying the
signal and building the response separately.

No behavior change: same response fields, same log messages on each surface.
