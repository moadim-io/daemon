---
"moadim": patch
---

feat(cli): print a one-time hint on a bare `moadim` start ("run `moadim install` to fix that") when the daemon isn't registered as an OS service yet, so it doesn't silently fail to survive a reboot or crash-restart. Non-interactive — no stdin prompt — and fires at most once per install, tracked via `~/.config/moadim/install_prompt.local.marker`.
