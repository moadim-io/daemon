---
"moadim": minor
---

Show a run the max-runtime watchdog killed as `killed`, not `unknown`. The runs API, the Runs page, and `moadim_runs_total` report it, and each run carries a `kill_reason` (`max_runtime`). Existing `unknown` and `failed` values keep their meaning. Runs killed before this change still show `unknown`, because their `exit_code` file holds only the `killed` marker. Rolling back to an older binary hides runs recorded as `killed` in `runs.log`: that binary skips lines whose status it does not know.
