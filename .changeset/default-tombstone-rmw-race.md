---
"moadim": patch
---

fix(routines): serialize the default-routine tombstone file's writes to close a lost-update race

`record_removed_default` and `clear_removed_default` each read the whole `removed_defaults.local.toml`
tombstone file, mutate the slug set, and write it back in full, with no synchronization between the
two. `DELETE /routines/{id}` and `POST /routines` (which call them from `svc_delete`/`svc_create`
respectively) can be handled concurrently on the multi-thread Tokio runtime, so two overlapping
read-modify-write round trips could interleave and the later write would silently drop whichever
change the other request had just persisted — e.g. deleting two different built-in default routines
back to back could lose one tombstone, resurrecting a routine the user explicitly removed on the
next daemon startup (the same hazard class as the crontab read-modify-write race fixed in issue
#365, and the `machine.local.toml` race fixed in #1240). Both functions now serialize through a
single `Mutex`, mirroring the existing `crontab_sync_lock`/`machine_toml_lock` pattern, so concurrent
tombstone writes can no longer clobber each other.
