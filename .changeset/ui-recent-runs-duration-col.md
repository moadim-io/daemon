---
"moadim": patch
---

feat(ui): add DURATION column to the Overview "RECENT RUNS" table

The fleet-wide recent-runs table on the Overview page previously showed
ROUTINE / STARTED / STATUS / EXIT CODE. It now also shows DURATION (wall-clock
elapsed between started_at and finished_at), matching the same column that
already exists on each routine's own HISTORY page.
