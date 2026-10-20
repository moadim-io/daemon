---
"moadim": patch
---

When two routine directories share one routine id, the daemon now loads the first in path order, skips the other with a warning naming it, and writes back to that same directory. Before, it silently loaded one copy and could write it into the other copy's directory, overwriting it.
