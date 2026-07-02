---
"moadim": minor
---

### Added

- **`--model` on the `moadim routines` CLI.** `create`, `update`, and
  `replace` gain a `--model <id>` flag, threaded into the same JSON body the
  REST route already accepts. The `model` field itself landed data/API-only
  in #742 with a note that other surfaces were a follow-up; this closes the
  gap for the terminal (the web UI form field remains a separate follow-up).
  On `update`, `--model ""` clears the override back to the agent's own
  default, matching the existing REST/MCP semantics.
