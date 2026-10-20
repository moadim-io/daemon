---
"moadim": minor
---

Add `moadim doctor`: a one-shot local check that `tmux` and `python3` are on `PATH`, that each agent config parses and its command resolves, and that the daemon is running. It prints one line per check and exits 1 if any check fails.
