# TODOs

> _Today's TODO, tomorrow's changelog. Ship one, dream up two._

This is a list of todos for consumption, in a pr remove the todo you have implemented and add any new ones you think of.

- Add a CI step (or weekly scheduled job) that checks whether a newer `crate-ci/typos` release exists than the pinned tag and opens/updates a tracking issue, so the pin gets bumped on a cadence instead of drifting silently
- Add a day-detail popover to the routines calendar: clicking a day lists each fire time (HH:MM) with its routine, and a "run now" shortcut per routine
- Have a commands folder for all the cli commands, we want to work with colocation of files
- Add a TTL preset row (1h / 1d / 7d / 30d) under the WORKBENCH TTL input in the routine form, mirroring the cron schedule presets
- Show a humanized retention countdown ("expires in 2d" / "expired") per finished run in the routine LOGS view, derived from the run's finish time and the routine's effective TTL
- Auto-refresh the routine LOGS view (or show a removed badge) after a CLEANUP NOW sweep so stale run output isn't shown for reaped workbenches
- Wire `make readme-check` (`scripts/check-readme-blocks.sh`) into `.github/workflows/lint.yml` as its own job — it validates README's fenced json blocks with jq but isn't CI-enforced yet, since adding it requires a token with the `workflow` OAuth scope
