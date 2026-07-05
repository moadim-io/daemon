---
"moadim": patch
---

fix: rotate daemon.log when it exceeds 10 MiB

`spawn_detached_with()` opened `daemon.log` in pure append mode with no
size cap, so a long-lived install could grow the file unbounded until it
filled the disk. Before opening the log, its size is now checked and
rotated to `daemon.log.1` past 10 MiB (best-effort — a failed rotation
falls through to the existing append-open rather than blocking the spawn).
