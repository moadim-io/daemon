---
"moadim": minor
---

### Added

- **Routine flags.** A routine's agent runs unattended inside tmux with no
  channel back to a human — until now. It (or a human, via MCP/HTTP) can
  raise a flag against a routine: a free-text `type` (e.g. `"bug"`, `"gap"`,
  `"edge_case"`, `"question"`) and free-text `description`, stored as
  `general` (committed) or `local` (gitignored) under the routine's
  `flags/` folder. New MCP tools `create_flag`, `list_flags`, `resolve_flag`
  and matching `/api/v1/routines/{id}/flags` REST endpoints. Open flags are
  injected into the routine's `prompt.md` on the next run so the agent sees
  what it flagged before, and the UI shows a flag-count badge with a
  read-only flags page to review and resolve them.
