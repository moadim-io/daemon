---
"moadim": minor
---

feat(ui): add a RELIABILITY page ranking routines by success rate, active failure streaks, and
flakiness

Adds a new `/reliability` tab to the dashboard that ranks every routine by its most recent 20
finished runs (issue #1256): success rate, active pass/fail streak, and a flakiness signal
(≥40% adjacent-run status flips) distinct from steadily-failing routines. Ranked worst-first — an
active failure streak outranks a merely-low historical success rate. Reads the existing fleet-wide
`GET /api/v1/routines/runs` endpoint (already used by the Routines table's sparkline column); no
backend change.
