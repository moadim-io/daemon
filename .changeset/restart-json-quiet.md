---
"moadim": minor
---

feat(cli): add `--json`/`--quiet` to `moadim restart`

`moadim restart` only printed human-readable status lines, so scripts had
no clean way to consume its result. `--json` now emits a single
machine-readable object (`{"old":N|null,"new":N,"address":…}`, matching the
shape every other `--json` lifecycle command surfaces), and `--quiet`
prints just the `restarted: pid <old> -> <new>` rotation line, suppressing
the UI/stop/logs hint block, for script-friendly output without the
overhead of JSON parsing.
