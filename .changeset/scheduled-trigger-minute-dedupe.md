---
"moadim": patch
---

Skip duplicate scheduled-trigger requests that target the same routine in the same minute, preventing overlapping multi-schedule cron expressions from launching duplicate workbenches.
