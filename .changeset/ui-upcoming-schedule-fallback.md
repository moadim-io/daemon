---
"moadim": patch
---

feat(ui): show raw cron in upcoming runs when no human description exists

The SCHEDULE column in the upcoming runs table previously showed "—" for
routines whose daemon had not yet computed a human-readable description.
It now falls back to the raw cron expression (e.g. `*/15 * * * *`) so
operators always see something actionable.
