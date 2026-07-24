---
"moadim": patch
---

fix(routines): kill in-flight routine tmux sessions on `moadim stop`, instead of leaving detached agents running with no watchdog until the next start's cleanup sweep (#320)
