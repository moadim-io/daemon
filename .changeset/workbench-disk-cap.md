---
"moadim": minor
---

Add `MOADIM_MAX_WORKBENCH_DISK_BYTES`, an optional total-disk ceiling for `~/.moadim/workbenches/`. The existing TTL sweep only reaps a workbench once it is old enough, so a handful of concurrent large runs (e.g. big repo clones) could exhaust the disk before any TTL elapsed (#398). Once set and exceeded, the same sweep now also evicts finished workbenches oldest-first — never a live session — until back under the cap. Unset or `0` keeps today's unbounded-by-size behavior.
