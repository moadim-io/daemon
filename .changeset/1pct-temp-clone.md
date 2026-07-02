---
"moadim": patch
---

### Fixed

- **"The 1 Percent" routine no longer mutates the live `~/.config/moadim` checkout.** Its PR step now clones the routines repo to a disposable temp directory and does all branch/commit/push work there, instead of running `checkout -b` / `commit` / `push` directly against the daemon's own routines checkout. This avoids leaving that checkout parked on a stale branch after merge and avoids racing the daemon's own reads of the routines folder.
