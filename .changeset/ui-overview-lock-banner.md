---
"moadim": patch
---

feat(ui): show global lock banner on the overview page

When routines are globally locked the OVERVIEW page now shows the same
warning banner as the Routines tab. Previously users on the overview had
no indication that scheduling and manual triggers were paused — they had
to navigate to another tab to discover the lock. The banner shows which
sentinels are active (SHARED .lock / LOCAL .local.lock) and is fetched
alongside the routine list on every refresh cycle.
