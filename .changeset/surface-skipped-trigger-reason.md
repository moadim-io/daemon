---
"moadim": patch
---

Fix a manual `trigger_routine` that gets skipped (agent load failure, an oversized inline
prompt, the overlap guard, or the global concurrency cap) surfacing no reason anywhere a caller
could see (#1145). `spawn_routine_command`'s skip branches now also append the reason to a new
per-routine `skip.log`, and `svc_logs` (the `routine_logs` backend) falls back to it when no
workbench was spawned, instead of coming back indistinguishable from "never triggered".
