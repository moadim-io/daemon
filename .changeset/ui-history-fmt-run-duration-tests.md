---
"moadim": patch
---

Add unit tests for the routine HISTORY page's `fmt_run_duration` formatter
and its run-status badge class/label helpers — they shipped untested,
including the `finished_at < started_at` underflow-guard branch.
