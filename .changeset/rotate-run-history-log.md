---
"moadim": patch
---

fix(routines): rotate a routine's `runs.log` instead of letting it grow forever

The reaper appends one durable [`PersistedRun`] record to a routine's
`runs.log` right before reaping its workbench, with no other trim point —
the same unbounded-growth shape already fixed for `daemon.log` (#316), just
scoped per routine instead of per daemon. A long-lived, frequently-firing
routine's history would otherwise grow without bound. `append_persisted_run`
now rotates `runs.log` to a sibling `runs.log.1` (replacing any previous one)
once it exceeds 1 MiB, mirroring `DAEMON_LOG_MAX_BYTES`'s rotate-and-replace
approach.
