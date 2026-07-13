---
"moadim": minor
---

feat(routines): auto-pull repositories into a persistent cache before each run

Before launching a routine's agent, the daemon now fetches + fast-forward pulls each of the routine's `repositories` into a persistent per-routine cache, so routines that rely on a fresh checkout no longer need to reinvent that sync logic. Opt out per routine with `auto_pull = false` in `routine.toml` (defaults to `true`). A pull failure (unreachable remote, diverged branch, unknown branch) never blocks the run — it raises a visible `auto_pull_failed` flag instead of failing silently.
