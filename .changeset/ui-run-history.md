---
"moadim": minor
---

feat(ui): add a run-history page for routines

Routines now record the exit code of every run (written to the workbench
by the launch command once the agent process exits) and expose it via
`GET /routines/{id}/runs` and `GET /routines/{id}/runs/{workbench}/log`.
A new HISTORY button on each routine row opens a page listing every kept
run — start time, status (RUNNING/SUCCESS/FAILED/UNKNOWN), duration, and
exit code — with a per-run log viewer, instead of the LOGS page's
newest-run-only view.
