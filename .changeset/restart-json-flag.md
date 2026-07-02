---
"moadim": minor
---

### Added

- **`moadim restart --json`.** Emits the PID-rotation summary as a
  machine-readable `{"old":N|null,"new":M}` object instead of the
  human-readable `restarted: pid <old> -> <new>` line, mirroring the
  `status`/`cleanup`/`stop` `--json` contract.
