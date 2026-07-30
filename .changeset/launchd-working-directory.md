---
"moadim": patch
---

Install the macOS LaunchAgent with a WorkingDirectory matching the directory where `moadim install` was run so launchd restarts keep the intended server root instead of defaulting to `/`.
