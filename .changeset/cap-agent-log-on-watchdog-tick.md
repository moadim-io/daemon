---
"moadim": patch
---

fix(routines): cap each workbench's `agent.log` to 32 MiB on the watchdog tick

`tmux pipe-pane -o` streams a session's raw pane output — every ANSI redraw
frame of a full-screen TUI agent included — into `agent.log` via an
unbounded, append-only `cat >>`. The `svc_logs`/`svc_run_log` read path
already bounds a single response to a 2 MiB tail (#280), but nothing bounded
the file's on-disk growth between TTL sweeps: a long-running or chatty
session could otherwise fill the disk before it was ever reaped (#268).

Adds `routines::cleanup::log_cap`, which truncates an oversized `agent.log`
in place to its last 32 MiB (prefixed with a marker noting how many bytes
were dropped) on the existing 30s watchdog tick, alongside the hung-session
kill check it already runs per workbench. Best-effort: an I/O failure for
one workbench is logged and does not abort the sweep for the rest.
