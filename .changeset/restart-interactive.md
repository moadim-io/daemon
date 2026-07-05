---
"moadim": minor
---

feat(cli): add `moadim restart --interactive/-i` to restart in the foreground

`moadim restart` always backgrounded the fresh instance, so restarting into
an attached, foreground session (to watch startup logs, or under a process
supervisor that expects a foreground child) required a separate `stop` +
`-i`. `restart -i`/`--interactive` now stops any running server, same as
`restart`, but brings the fresh instance up in the foreground instead of
detaching it — mirroring `moadim -i`.
