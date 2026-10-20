---
"moadim": patch
---

`moadim doctor` now flags an active global lock (which file, since when) with the `moadim unlock` hint, since a forgotten lock silently halts every routine.
