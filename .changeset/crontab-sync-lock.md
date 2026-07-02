---
"moadim": patch
---

Serialize the crontab read-modify-write across concurrent syncs so overlapping `crontab -l` → edit → `crontab -` round trips can no longer interleave and clobber each other's writes.
