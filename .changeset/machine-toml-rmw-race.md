---
"moadim": patch
---

fix(machine): serialize `machine.local.toml` writes to close a lost-update race

`set_machine` and `set_max_concurrent_runs_override` each read the whole `machine.local.toml`,
mutate one field, and write the whole struct back, with no synchronization between the two. `PUT
/machine` and `PUT /config/max-concurrent-runs` can be handled concurrently on the multi-thread
Tokio runtime, so two overlapping read-modify-write round trips could interleave and the later
write would silently drop whichever field the other request had just persisted (the same hazard
class as the crontab read-modify-write race fixed in issue #365). Both functions now serialize
through a single `Mutex`, mirroring the existing `crontab_sync_lock` pattern, so a concurrent
machine-name rename and concurrency-cap update can no longer clobber each other.
