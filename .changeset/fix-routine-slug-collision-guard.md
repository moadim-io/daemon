---
"moadim": patch
---

fix(routines): guard `write_routine` against a stale on-disk slug collision (#188)

Two distinct routine titles can slugify to the same folder name (e.g. `"Update deps!"` and `"Update deps?"` both become `update-deps`). The in-memory create/update handlers already reject that when both routines are loaded, but a slug could also collide with a stale `routine.toml` left on disk by something outside the in-memory store (e.g. a directory `remove_routine_dir` failed to clean up) — and `write_routine` would silently overwrite it, including the wrong `prompt.md` a scheduled run then executes. `write_routine` now checks the target slug's existing `routine.toml` id before writing and refuses to overwrite a different routine's files, surfaced as a 409 Conflict instead of a 500 at the `create`/`update` API handlers.
