---
"moadim": patch
---

Stop leaking per-routine env secrets through process arguments. Values from `routine.toml` `[env]` and `routine.local.toml` used to be inlined into the launch command as `export KEY='value'`, readable by any local user via `ps` while the wrapper shell ran. They now reach the routine through the spawned process's own environment, and the command only names the variable.
