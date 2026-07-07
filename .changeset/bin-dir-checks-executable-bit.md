---
"moadim": patch
---

### Fixed

`agent_command_available`/`tmux_available` (surfaced via `GET /health` and `RoutineResponse.agent_command_available`) now require the executable bit before reporting a `PATH` binary as available, instead of only checking `Path::is_file()`. A regular, non-executable file named `tmux`/the agent `command` (a broken install, an untarred archive, a `chmod`-stripped copy) previously passed as "available," so the UI/API showed a routine as healthy even though the actual cron firing (`sh -c '<bin> …'`) would fail with "Permission denied" and silently no-op — exactly the failure mode these checks exist to catch.
