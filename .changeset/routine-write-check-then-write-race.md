---
"moadim": patch
---

fix(routines): serialize `write_routine`'s check-then-write span to close a directory-corruption race

`write_routine` reads the target slug's on-disk `routine.toml` for a collision check (guarding
against two distinct titles slugifying to the same folder name, issue #188), then writes
`routine.toml`, the two prompt sidecars, and `state.local.toml`, with no synchronization against
another overlapping call. `POST /routines` and `PATCH /routines/{id}` can be handled concurrently
on the multi-thread Tokio runtime, so two calls whose titles collide could each pass the collision
check before either had written anything, then interleave their four sequential file writes into
the same directory — silently leaving a mix of files from both routines (e.g. `routine.toml` from
one, `state.local.toml` from the other), exactly the outcome the collision check exists to prevent
but cannot on its own because the check and the writes aren't atomic together. Same hazard class as
the `machine.local.toml` and tombstone-file read-modify-write races already fixed in this codebase.
`write_routine` now serializes through a single `Mutex`, mirroring the existing
`machine_toml_lock`/`crontab_sync_lock` pattern, so a losing call's collision check always observes
the winning call's fully-written result and cleanly errors out instead of writing at all.
