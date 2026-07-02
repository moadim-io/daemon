---
"moadim": patch
---

### Fixed

- **Default routines with empty `machines` list now self-repair.** Default routines
  seeded before machine-awareness was introduced could be left permanently dormant
  (empty `machines` list, so no machine ever matched them). The daemon now detects
  an empty machines list during the startup reconcile pass and seeds the current
  machine, restoring the routine to an active state without any manual intervention.
  (#723)
