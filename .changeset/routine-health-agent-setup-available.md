---
"moadim": patch
---

fix: surface a routine's unresolvable `setup`-step interpreter as unhealthy instead of green

`GET /api/v1/routines` responses (`RoutineResponse`) now carry `agent_setup_available`, mirroring
the existing `agent_command_available` field: `true` unless the agent config has a `setup` step
whose first token (the interpreter it shells out to, e.g. `python3` for the built-in `claude`
agent's workspace-trust seeding) does not resolve on the daemon's `PATH`.

Closes the remaining gap from issue #404: `GET /health`'s `dependencies.python3` flag (added in
#902) already told the operator the daemon-wide dependency was missing, but a routine using the
`claude` agent still showed a green "healthy" dot even though its `setup` step — and therefore
the whole run — was guaranteed to fail before the agent ever launched. The UI's `routineHealth()`
now folds `agent_setup_available` into the same "AGENT MISSING" badge `agent_registered` already
uses, since both mean the run aborts without the agent starting.
