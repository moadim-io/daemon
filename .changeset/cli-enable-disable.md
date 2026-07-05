---
"moadim": minor
---

feat(cli): add `moadim enable`/`disable <routine>` to flip enabled from the terminal

Toggling a routine's `enabled` state previously required a raw `moadim
routines update <id> --enabled true/false` call or the web UI. `moadim
enable <routine>` / `moadim disable <routine>` now flip it directly (by id
or slug, resolved server-side), printing a human status line or a
`{"routine","enabled"}` object under `--json`. Both are idempotent: setting
an already-enabled routine to enabled again is a no-op success, not an
error.
