---
"moadim": patch
---

feat(ui): add Health option to routines group-by selector

The routines GROUP BY selector now includes "Health" as an option.
Choosing it partitions the routine list by the derived health badge
(HEALTHY, SNOOZED, DORMANT, DEAD SCHEDULE, AGENT MISSING, DISABLED),
making it easy to scan which routines share the same health state.
