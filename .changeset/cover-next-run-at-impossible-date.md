---
"moadim": patch
---

Add a unit test covering `next_run_at`'s "no upcoming fire" branch (an impossible calendar date like `0 0 30 2 *`, which parses fine but never fires), closing one of the gaps in the repo's 100%-line-coverage floor.
