---
"moadim": patch
---

Retry the `on_failure` webhook on transient delivery failures. A failed `curl` POST is now tried up to 3 times with exponential backoff (100 ms, then 200 ms) before the failure is logged, so one dropped request no longer loses a routine's failure notification.
