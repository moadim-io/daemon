---
"moadim": patch
---

feat(ui): add UNLOCK ALL button to the overview page lock banner

The overview page's lock banner previously showed "ROUTINES GLOBALLY
LOCKED" as a read-only notice. It now renders the same `GlobalLockBanner`
component used on the Routines page, which includes an UNLOCK ALL button.
Operators no longer need to navigate to the Routines tab to clear a lock.
