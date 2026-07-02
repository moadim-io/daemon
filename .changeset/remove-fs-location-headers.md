---
"moadim": patch
---

Remove the `fs_location` middleware (issue #356) that injected `x-server-root` / `x-server-exe-dir` headers, containing the daemon's absolute working-directory and executable paths, into **every** HTTP response. Nothing consumed these headers — the CLI reads JSON response bodies, and the shipped UI has zero references to them — so they were pure information-disclosure surface (OS username + filesystem layout) with no functional dependent. The same `FsLocation` data remains available to intentional callers via `GET /api/v1/health` and the MCP `health` tool.
