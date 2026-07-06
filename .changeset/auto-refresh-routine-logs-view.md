---
"moadim": patch
---

### Fixed

Auto-refresh the routine LOGS view on the same operator-chosen cadence already used by the routines list (via the shared `AUTO` interval control), instead of only reloading once on mount. Previously, a workbench reaped by the periodic background cleanup sweep while a run's LOGS page was open left stale, already-deleted output on screen until the operator remembered to click the manual "↻" button (#357).
