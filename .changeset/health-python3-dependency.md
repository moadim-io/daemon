---
"moadim": patch
---

### Added

`GET /health`'s `dependencies` now also reports `python3` (alongside the existing `tmux` flag), and the daemon logs a startup warning when it is missing. The built-in `claude` agent's `setup` step depends on `python3` to pre-seed workspace-trust state; previously a missing `python3` failed that step silently, with the routine still showing a healthy status.
