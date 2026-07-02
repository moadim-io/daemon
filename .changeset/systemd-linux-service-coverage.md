---
"moadim": patch
---

### Changed

- **Closed the Linux systemd-service test gap that was failing `cargo llvm-cov (100% line floor)` on every PR.** `service::linux` (systemd user-unit install/uninstall) had no test seam for `systemctl` and almost no tests, so on the Linux CI runner it sat at ~17% line coverage while `service::macos` (fully seamed and tested) sat at 100% — tripping the repo-wide 100%-line floor and blocking merges regardless of what a PR actually touched. Added a `MOADIM_SYSTEMCTL_BIN` seam mirroring macOS's `MOADIM_LAUNCHCTL_BIN`, split `unit_path()` into a directly-testable `unit_path_from_config_dir()`, and added install/uninstall/write-unit tests mirroring the existing macOS coverage. No behavior change on either platform.
