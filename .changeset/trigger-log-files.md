---
"moadim": minor
---

### Changed

- **Scheduled and manual trigger history is now recorded in append-only `.log` files.** `scheduled.local.toml` (overwritten on each cron fire) is replaced by `scheduled.log`; the manual-trigger timestamp previously stored in `state.local.toml` moves to `manual.log`. Each file records one Unix timestamp per execution, giving a full run history instead of only the most recent timestamp. A startup migration seeds the log files from any legacy TOML sidecars found on disk and removes the old files, so existing installs upgrade transparently. The `.log` suffix matches the existing `*.log` gitignore pattern seeded into each routine directory.
