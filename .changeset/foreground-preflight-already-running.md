---
"moadim": patch
---

### Fixed

- **`moadim start` (foreground) could clobber an already-running daemon.** Running `moadim start` in the foreground while a background daemon was already up used to proceed anyway instead of failing fast. It now preflights with `ensure_not_running_for_foreground()` and exits with a clear error before binding, matching the existing background-start behavior.
