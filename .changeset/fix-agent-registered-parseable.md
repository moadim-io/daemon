---
"moadim": patch
---

fix(routines): derive `agent_registered` from parseability, not file existence

`RoutineResponse.agent_registered` was `true` whenever `<agent>.toml` merely existed on disk, even
if it was malformed. Crontab sync drops such routines via `load_agent_command`, so they never fire
— but the API reported them as healthy. `agent_registered` is now `load_agent_command(...).is_ok()`,
matching what sync actually requires.
