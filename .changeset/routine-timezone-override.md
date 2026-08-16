---
"moadim": patch
---

Add an optional per-routine `timezone` (IANA name) so a schedule can pin to a zone instead of always running in the host crontab's own zone. Applied via a `CRON_TZ` directive in the managed crontab block; only accepted on Linux hosts, since BSD `cron` (macOS) does not honor `CRON_TZ`.
