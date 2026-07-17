---
"moadim": patch
---

fix(routines): serialize flag-creation's collision-check-then-write span to close a lost-update race

`create_flag` reads the routine's `flags/` directory to find a free `{type}-{timestamp}.md`
filename, then writes to it, with no synchronization between the check and the write. The HTTP and
MCP flag-creation handlers can be invoked concurrently on the multi-thread Tokio runtime, so two
overlapping calls for the same routine and flag type could both observe the same candidate filename
as free before either writes, and whichever write lands second would silently clobber the first —
directly contradicting `create_flag`'s own doc comment that "a flag never silently overwrites
another" (the same hazard class as the crontab, `machine.local.toml`, and default-tombstone
read-modify-write races fixed in issues #365, #1240, and #1243). `create_flag` now serializes
through a single `Mutex`, mirroring the existing `crontab_sync_lock`/`machine_toml_lock` pattern, so
concurrent flag creation can no longer clobber another in-flight flag.
