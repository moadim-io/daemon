---
"moadim": patch
---

refactor(routes): move health HTTP + MCP endpoints into `routes/health`

Splits `src/routes/health/` into `mod.rs` (wiring), `logic.rs` (shared `HealthResponse` /
`DependencyHealth` types and the `build()` function), `http.rs` (the `GET /health` handler), and
`mcp.rs` (the MCP `health` tool, declared as a child module of `routes::mcp` so it keeps access to
`MoadimMcp`'s private state). The MCP tool now builds on the shared `logic::build()` instead of
re-deriving status/uptime/dependencies/version by hand, so the two surfaces can't drift.

No behavior change: same response fields on both `GET /health` and the MCP `health` tool.
