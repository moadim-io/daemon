---
"moadim": patch
---

Fix the built-in "Token Trim" routine's PR step so it clones `~/.config/moadim`'s origin into a disposable `mktemp -d` temp dir and does all branch/commit/push work there, instead of checking out a branch directly inside `~/.config/moadim` — the live checkout the daemon reads routines from. This matches the fix already shipped for the sibling "The 1 Percent" routine (#916); "Token Trim" was never updated to the same pattern, so it still risked leaving the daemon's routines checkout parked on a stale branch mid-run.
