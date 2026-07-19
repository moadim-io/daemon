---
"moadim": patch
---

fix(routines): stop `runs.log` rotation from silently discarding prior run history (#1277)

A routine's durable `runs.log` is rotated to a sibling `runs.log.1` once it
crosses 1 MiB (`RUN_HISTORY_MAX_BYTES`), but the rotation used a bare
`fs::rename`, which **overwrites** any existing `.1` file. Combined with
`read_persisted_runs` only ever reading the current `runs.log`, every
rotation past the first permanently discarded that routine's history —
despite `runs.log` being documented as durable history that survives
workbench TTL reaping.

- `rotate_run_history_if_oversized` now merges the rotating-out content onto
  the end of any existing `.1` file instead of overwriting it.
- `read_persisted_runs` now reads both `runs.log` and `runs.log.1` and
  merges the results, so `GET /routines/{id}/runs` / `GET /routines/runs`
  and the UI views built on them (history, Overview, Reliability rankings)
  no longer lose history across a rotation.
- Added a regression test exercising rotation followed by a read, asserting
  pre-rotation entries are still visible afterward.
