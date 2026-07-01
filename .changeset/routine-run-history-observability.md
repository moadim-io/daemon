---
"moadim": minor
---

### Added

- **Run history for routines.** `GET /routines/{id}/runs` (and the `list_routine_runs` MCP
  tool) lists a routine's past and in-progress runs — derived from its workbench directories,
  newest first — with each run's start time, whether it's still running, and its exit code.
  `GET /routines/{id}/logs` gained an optional `?run=<id>` query parameter to fetch a specific
  run's log instead of only the newest. The Logs page in the UI now has a RUNS tab listing
  these with a status badge; clicking a run shows its log.
