---
"moadim": patch
---

Stop logging `routine scheduler resync request failed: routine scheduler is not running` on every daemon start. The routine scheduler now starts before the startup sync requests its first resync.
