---
"moadim": minor
---

### Added

- **`agent_command_available` on routine responses.** `RoutineResponse` now
  reports whether the routine's agent `command` (e.g. `claude`, `codex`)
  actually resolves on the daemon's `PATH`, distinct from the existing
  `agent_registered` (which only checks that `<agent>.toml` exists). A
  routine with a present, well-formed agent config but an uninstalled binary
  previously looked identically healthy to one that could actually run.
