---
"moadim": patch
---

test(routines): cover `rotate_run_history_if_oversized`'s rotated-write-failure branch — when the `.1` sibling can't be written (e.g. it turns out to be a directory), the oversized source `runs.log` must survive rather than being removed, since removal is gated on the write actually succeeding. This branch previously had no test, unlike its sibling I/O-failure branches elsewhere in `cleanup::log_cap`.
