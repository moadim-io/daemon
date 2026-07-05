---
"moadim": patch
---

feat(ui): add MACHINES column to the routines table

The routines table now has a MACHINES column showing how many machines
each routine is assigned to. When a routine has no machines assigned
(dormant) the cell shows an amber "—" instead of a number, so operators
can spot un-targeted routines without filtering. Hovering the count
shows the full list of machine names.
