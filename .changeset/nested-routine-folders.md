---
"moadim": minor
---

feat(routines): support nested routine folders — a routine's `title` may contain `/`-separated
path segments (e.g. `ops/nightly triage`), organizing routines into folders and subfolders in the
UI. `slugify` now preserves `/` as a segment separator instead of collapsing it, so nested titles
produce nested, filesystem- and tmux-safe slugs.
