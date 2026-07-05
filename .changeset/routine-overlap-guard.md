---
"moadim": patch
---

### Fixed

A routine had no overlap guard: nothing stopped a new fire from launching while a previous fire of the *same* routine was still running. A routine whose agent run outlived its schedule interval (e.g. `* * * * *` with a slow agent) would pile up concurrent tmux sessions all acting on the same target — duplicate PRs/issues, racing git pushes. Both the manual and scheduled trigger paths now check for a live tmux session under the routine's `moadim-<slug>-` prefix before launching, and skip the fire (with a warning) if one is still active.
