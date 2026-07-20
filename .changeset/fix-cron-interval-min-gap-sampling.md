---
"moadim": patch
---

fix(cleanup): `cron_interval_secs` now samples multiple fires to find the schedule's true minimum gap, instead of just the next two — fixes TTL/max-runtime ceilings silently flipping between values depending on wall-clock time for unevenly-spaced multi-fire-per-day schedules (e.g. `"0,30 9 * * *"`).
