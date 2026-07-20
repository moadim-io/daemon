---
"moadim": patch
---

Add an opt-in "Notify on failure" toggle to the Overview page's Recent Runs
section: requests browser notification permission once enabled, then fires a
desktop notification the moment a polled run transitions to `failed` (no
notification for failures already in view when you turn it on).
