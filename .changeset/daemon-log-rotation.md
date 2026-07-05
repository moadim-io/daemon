---
"moadim": patch
---

Rotate `daemon.log` to a `.log.1` sibling once it exceeds the size cap instead of letting it grow forever — a daemon meant to run unattended for weeks/months must not silently fill the disk. Adds focused unit test coverage for `rotate_daemon_log_if_oversized` (missing file, small file, oversized file, replacing a stale `.1`).
