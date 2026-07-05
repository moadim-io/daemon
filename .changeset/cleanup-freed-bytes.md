---
"moadim": minor
---

feat(cleanup): report freed disk bytes alongside removed count

`POST /routines/cleanup` (and `moadim cleanup`, the `cleanup_workbenches`
MCP tool, and the web UI's CLEANUP NOW button) now report how much disk
space a sweep reclaimed, not just how many workbenches it removed: each
reaped workbench's tree is measured just before deletion and summed into
a new `freed_bytes` field on `CleanupResponse` (additive — existing
`{"removed": N}` consumers are unaffected). `moadim cleanup` prints
`cleanup removed N workbench(es) (freed 12.4 MB)`, and the UI's cleanup
toast mirrors the same humanized size.
