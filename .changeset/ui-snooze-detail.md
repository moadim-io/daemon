---
"moadim": patch
---

feat(ui): show snooze-until detail in the NEXT RUN cell

Snoozed routines previously showed only "snoozed" in the NEXT RUN
column. The cell now includes a secondary line with context:

- "Nm left" / "Nh left" / "Nd left" — when a `snoozed_until` deadline
  is set, showing how long until the routine resumes automatically.
- "N run(s) skipped" — when a `skip_runs` counter is active.

Seven new host tests cover all the formatting branches.
