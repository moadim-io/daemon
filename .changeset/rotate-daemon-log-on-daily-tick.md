---
"moadim": patch
---

fix(cli): rotate daemon.log on a daily tick, not just at spawn

`rotate_daemon_log_if_oversized` only rotated at detached-spawn time or on size, so a
long-lived daemon that stayed under the size cap and never restarted never rotated its log.
Renamed to `rotate_daemon_log_if_due` and added a 24h age-based trigger alongside the size
check, re-evaluated hourly via a new periodic task in `run_with_listener_until`.
