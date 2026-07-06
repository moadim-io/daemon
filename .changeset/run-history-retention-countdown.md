---
"moadim": patch
---

feat(routines): show a humanized retention countdown per finished run in the
run-history view

`RunSummary` now carries `retention_expires_at` (finish time + the routine's
effective TTL) for runs whose workbench is still on disk. The HISTORY page
renders it as a `RETENTION` column ("expires in 12m" / "expired"), so users
can see how long a finished run's log stays before cleanup reaps it, instead
of guessing from the TTL alone (#477).
