---
"moadim": minor
---

### Added

- **Optional per-routine model override.** `Routine`, `CreateRoutineRequest`,
  `UpdateRoutineRequest`, persisted `RoutineToml`, and the MCP
  `UpdateRoutineInput` all gain a `model: Option<String>` field, blank/whitespace
  normalized to `None` (agent's own default). `build_routine_command` appends
  `--model <id>` (shell-quoted) to the agent invocation when set, after the
  agent's own args so it wins over any default. Defaults reconciliation treats
  `model` as user-owned, like `tags`: never overridden by a built-in routine's
  spec. Scoped to the data/API layer for now; the web UI form field is a
  follow-up. (#742)
