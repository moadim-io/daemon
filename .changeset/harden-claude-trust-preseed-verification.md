---
"moadim": patch
---

fix(routines): verify the Claude trust-dialog pre-seed actually persisted before launching

The `claude` agent's `setup` step pre-seeds `~/.claude.json` so a headless routine run
never blocks on Claude Code's "Do you trust this folder?" dialog. Live runs were found
parked at that exact dialog for hours — reaped only by the ~1h watchdog, with an empty
log — because the write had silently not taken effect for that workbench. The setup
script now reads `~/.claude.json` back after writing and asserts the seeded entry is
actually there; a failed assertion makes the script exit non-zero, which the launcher's
existing `{setup}; } || { ...; exit 1; }` guard turns into an immediate, diagnosable
"agent setup failed" abort instead of a silent multi-hour hang.
