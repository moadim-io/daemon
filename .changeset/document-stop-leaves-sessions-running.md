---
"moadim": patch
---

docs(cli): document that `moadim stop` does not kill in-flight routine sessions

`moadim stop` (and the UI STOP button / `POST /shutdown`) only stops the
daemon's own HTTP/MCP server. Routine agents run in a **detached** tmux
session (`tmux new-session -d`), independent of the daemon process, so an
in-flight run is never touched by a stop request — it keeps running (and can
keep opening PRs, filing issues, pushing commits, etc.) until it finishes on
its own or a later daemon start's watchdog/cleanup sweep reaps it (#320).

This behavior was previously undocumented, so `moadim stop` reporting
success could read as "everything stopped" when a routine agent was still
acting. Documents it in `moadim --help`, the `Command::Stop`/`stop()` doc
comments, `README.md`, `Architecture.md`, and `docs/moadim.1` — no behavior
change.
