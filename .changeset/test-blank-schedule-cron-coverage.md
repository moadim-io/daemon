---
"moadim": patch
---

test(routine_storage): cover `read_routine_cron`'s empty-file branch — a `schedule.cron` sidecar with no non-empty line (e.g. truncated by a crash mid-write) now has a regression test asserting the whole routine load short-circuits to `None` instead of silently loading with a blank schedule, closing a gap in the pre-push 100%-line-coverage gate.
