---
"moadim": patch
---

`moadim doctor` now also flags an agent whose `setup` step shells out to a tool that is not on `PATH` (e.g. `python3` for the built-in `claude` agent).
