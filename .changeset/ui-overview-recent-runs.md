---
"moadim": minor
---

feat(ui): add a fleet-wide RECENT RUNS panel to the overview page

`GET /routines/runs?limit=N` returns the most recent runs across every
routine (newest first, one workbench-directory scan) instead of one
`/routines/{id}/runs` request per routine. The overview page's new RECENT
RUNS table uses it to show what just ran fleet-wide, complementing the
existing UPCOMING RUNS panel (future fires) with the equivalent view of
the past.
