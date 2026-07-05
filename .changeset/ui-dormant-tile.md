---
"moadim": patch
---

feat(ui): add DORMANT tile to routines stats bar

The routines stats bar now shows a DORMANT tile — the count of enabled
routines assigned to no machine (they are enabled but will never fire).
The tile turns amber when any dormant routines exist and acts as a
clickable filter, narrowing the table to dormant routines.
