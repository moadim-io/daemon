---
"moadim": minor
---

### Added

- **`list_routine_runs` MCP tool.** Exposes the existing `GET /routines/{id}/runs` run-history
  endpoint over MCP, so an agent can list a routine's past and in-progress runs (workbench id,
  start/finish time, status, exit code) the same way it already can over REST — without needing
  a separate call per run just to fetch `routine_logs`' newest-only log.
