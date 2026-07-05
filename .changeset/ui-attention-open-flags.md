---
"moadim": patch
---

feat(ui): surface routines with open flags in the NEEDS ATTENTION panel

The overview's NEEDS ATTENTION panel caught config problems (DORMANT,
DEAD SCHEDULE, AGENT MISSING) but was blind to runtime issues: routines
whose agents raised flags during a run never appeared there. Operators
had to discover flagged routines by scanning the routines table.

Adds HasOpenFlags as an attention reason (rank 3, lowest priority so
config faults still surface first). An enabled routine with flag_count > 0
now appears in the panel with an "OPEN FLAGS — agent raised flags during
a run — needs review" badge.

Three new tests: open flags surfaces when otherwise healthy, config
faults outrank flags, disabled routines with flags remain hidden.
