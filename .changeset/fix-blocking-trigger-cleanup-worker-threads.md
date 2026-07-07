---
"moadim": patch
---

Move `POST /routines/{id}/trigger`, `/scheduled-trigger`, and `/routines/cleanup` off the async worker thread. These handlers call `svc_trigger`/`svc_trigger_scheduled`/`svc_cleanup`, which shell out to `tmux`(1) and do blocking filesystem I/O; they previously ran inline on the Tokio worker thread instead of `spawn_blocking`, unlike the sibling create/update/delete/lock/unlock handlers. A hung `tmux` call (or a `*/N` scheduled-trigger herd) could stall unrelated requests such as `GET /health`.
