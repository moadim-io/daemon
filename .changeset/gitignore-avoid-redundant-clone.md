---
"moadim": patch
---

Avoid a redundant `String` clone in `ensure_config_gitignore()`: `existing` is only borrowed (via `lines()`) before the clone site and is never read again afterward, so the buffer can be moved into `content` instead of cloned. No behavior change; this runs on every daemon start/restart.
