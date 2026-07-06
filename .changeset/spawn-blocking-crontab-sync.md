---
"moadim": patch
---

### Fixed

Run crontab sync (`lock`/`unlock`/`create`/`update`/`delete` on `/routines`) via `tokio::task::spawn_blocking` instead of inline on the async handler. These calls shell out to `crontab`(1); without `spawn_blocking` a slow or hung `crontab` invocation pins a Tokio worker thread indefinitely, and the per-request 30s timeout (`middlewares/timeout.rs`) can't preempt it since the thread is synchronously blocked, not polling a future (#360).
