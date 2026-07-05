---
"moadim": patch
---

feat(ui): add DORMANT KPI tile to the overview page

The overview KPI row now includes a DORMANT tile — the count of enabled
routines assigned to no machine (they are enabled but will never fire).
The tile turns amber when any dormant routines exist, matching the DORMANT
tile already present on the Routines page stats bar.
