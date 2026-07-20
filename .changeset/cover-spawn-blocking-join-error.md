---
"moadim": patch
---

test: cover the `spawn_blocking` join-failure branch shared by routine route handlers

`create_routine`, `delete_routine`, `lock_routines`, `trigger_routine`, `unlock_routines`,
`update_routine`, and the scheduled-trigger handler each repeated the same
`tokio::task::spawn_blocking(..).await.map_err(|_| AppError::Internal)??` idiom (#360) to keep
blocking `crontab`/`tmux`/filesystem I/O off the async worker thread. The `map_err` branch — hit
only when the blocking task itself panics — was duplicated seven times and untested at every call
site, since the poison-tolerant stores (`LockRecover`) never actually panic through normal use.
That left `cargo llvm-cov --fail-under-lines 100` (the repo's own CI and pre-push gate) short of
100%.

Extracts the idiom into one `crate::error::run_blocking` helper used by all seven call sites, and
adds direct unit tests for it (including one that deliberately panics the blocking closure) so the
branch is written, and covered, exactly once. No behavior change; purely a dedup + coverage fix.
