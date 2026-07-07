---
"moadim": patch
---

### Added

A shared cron minute (e.g. `*/5 * * * *`, `0 * * * *`) could launch an unbounded thundering herd of agent sessions: each routine fire spawns its own detached tmux session with no cap on how many may be alive across *all* routines at once — distinct from the existing per-routine overlap guard, which only stops one routine from stacking on its own still-running fire. `MOADIM_MAX_CONCURRENT_RUNS` (default `4`) now caps the number of concurrently-running routine agent sessions; a fire that would exceed it is skipped (logged, not queued) and picked up again on its next scheduled tick. The live count is derived from actual tmux session liveness, not an in-memory counter, so it stays correct across a daemon crash/restart.
