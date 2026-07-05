---
"moadim": patch
---

fix(routines): collision-resistant run id so same-second runs don't clobber

Two runs of the same routine landing in the same wall-clock second (a
double-clicked "Run now", a `trigger` retry, or a manual trigger racing the
scheduled cron fire) derived an identical `$WB`/`$SESS` from `$TS`'s
one-second granularity — the second `tmux new-session` failed with
"duplicate session" and silently no-opped while both clobbered the shared
workbench files. The launch script now mixes the launching shell's PID
(`$$`) into the run id (`$TS_$$`), and fails loudly instead of silently
no-opping if `tmux new-session` still collides. (#411)
