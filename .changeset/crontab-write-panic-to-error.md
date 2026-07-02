---
"moadim": patch
---

### Fixed

- **A crontab-sync write failure panicked the daemon instead of degrading gracefully.** `write_crontab` piped the routine schedule into `crontab -` and `.expect()`'d both the stdin write and the child's exit status. If the external `crontab` process ever closed its end of the pipe early (e.g. it rejects malformed input mid-stream), the write failed with a broken-pipe error that panicked the request thread — even though every caller of crontab sync already treats a `SyncError` as warn-and-continue, not fatal. Both failure paths now propagate a `SyncError::Io` instead of panicking, and the child is always reaped via `wait()` even when the write fails.
