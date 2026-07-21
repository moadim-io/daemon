---
"moadim": patch
---

Restore the `schedule` field in `routine.toml`. The schedule-to-`schedule.cron` split made the sidecar the source of truth prematurely; `routine.toml` now carries the authoritative `schedule` again (written and read first), while the `schedule.cron` sidecar keeps being written as a mirror of the cron entry (not functional yet). Dirs written during the sidecar-only era still load via a cron-file fallback (comment lines are skipped) until the next repersist heals them, and the JSON Schema requires `schedule` again.
