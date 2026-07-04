---
"moadim": patch
---

fix(ui): show "snoozed" in NEXT RUN cell instead of suppressed fire time

When a routine is snoozed its scheduled fires are suppressed, but the
NEXT RUN column still showed the upcoming time as if the run would happen.
Now shows "snoozed" (muted, consistent with "paused" for disabled
routines) so the table accurately reflects what will execute.

Extracts `is_routine_snoozed` as a shared helper used by both
`routine_health` and `next_routine_run_cell`, with four dedicated tests.
