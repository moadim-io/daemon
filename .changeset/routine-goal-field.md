---
"moadim": minor
---

### Added

- **Optional `goal` for routines.** A routine can now carry a very short (at most
  5 lines) statement of its goal — the "why" behind the prompt. It is optional
  (default unset), persisted in the tracked `routine.toml`, and rendered into the
  agent's `prompt.md` as a `## Goal` preamble ahead of the task. Settable across
  every surface: REST (`goal` on the create/update bodies), MCP
  (`create_routine`/`update_routine`), the CLI (`--goal` on
  `routines create|replace|update`), and the web UI (a field in the routine
  form). The value is trimmed; a goal longer than 5 lines is rejected with
  `400 Bad Request`, and sending an empty string on update clears it. The three
  built-in default routines now ship with a goal. (#827)
