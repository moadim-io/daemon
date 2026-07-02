---
"moadim": patch
---

### Fixed

- **Renaming a routine no longer strands its prior run history under the old slug.** Workbenches (`~/.moadim/workbenches/{slug}-{ts}`) are keyed by a routine's title slug, not its stable id. `PATCH`/`PUT /routines/{id}` now migrates every existing `{old_slug}-{ts}` workbench to `{new_slug}-{ts}` when the title changes, so `GET /routines/{id}/logs` keeps finding prior runs and the cleanup watchdog keeps resolving an in-flight run to the renamed routine's own `ttl_secs`/`max_runtime_secs` instead of falling back to orphan defaults (#267).
