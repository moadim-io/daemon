# TODOs

> _Today's TODO, tomorrow's changelog. Ship one, dream up two._

This is a list of todos for consumption, in a pr remove the todo you have implemented and add any new ones you think of.

- Add a `cargo xtask spellcheck` (or `make spell`) wrapper that installs and runs `typos` so contributors don't need to know the tool name
- Add a `fmt + clippy` CI job (mirroring the pre-push hook: `cargo fmt --check`, `cargo clippy -- -D warnings`) so style/lint regressions are caught in PRs without local hooks
- Add a CI step (or weekly scheduled job) that checks whether a newer `crate-ci/typos` release exists than the pinned tag and opens/updates a tracking issue, so the pin gets bumped on a cadence instead of drifting silently
- Pin the remaining third-party GitHub Actions (e.g. `actions/checkout@v5`) to full commit SHAs, mirroring the typos pin, so every CI dependency is reproducible
- Add a day-detail popover to the routines calendar: clicking a day lists each fire time (HH:MM) with its routine, and a "run now" shortcut per routine
- Link the routines calendar UI to the new `GET /routines.ics` feed: add a "SUBSCRIBE" button that copies the feed URL, plus a per-routine `?routine=<id>` filter on the endpoint
- Fold the host's local timezone into the `.ics` feed as a `VTIMEZONE` block with `DTSTART;TZID=` local times, so subscribers see fire times in the routine's own zone instead of only UTC
- Auto-stamp the release version/date into CHANGELOG.md from the `chore(release)` step so the `## [Unreleased]` section rolls over on tag
- Add a CI check that fails a PR when it changes `src/` or `ui/` but leaves `## [Unreleased]` in CHANGELOG.md empty
- Have a commands folder for all the cli commands, we want to work with colocation of files
- Dismiss any open UI modal/dialog (edit, delete-confirm, shutdown-confirm) with the Esc key
- Show server uptime as a humanized duration (e.g. "2h 14m") next to the health badge in the header
- Have better daemon logging, with timestamps and log levels
- Add a TTL preset row (1h / 1d / 7d / 30d) under the WORKBENCH TTL input in the routine form, mirroring the cron schedule presets
- Show a humanized retention countdown ("expires in 2d" / "expired") per finished run in the routine LOGS view, derived from the run's finish time and the routine's effective TTL
- Add a `--json` flag to `moadim status`/`cleanup` so the CLI output can be consumed by scripts
- Add a `moadim restart --interactive` (or `-i`) flavor that restarts the daemon in the foreground attached to the terminal instead of detached, mirroring `moadim -i`
- Have `moadim restart` print the old PID it stopped and the new PID it started (e.g. "restarted: pid 123 -> 456") so scripts/logs can see the process actually rotated
- Return the freed disk bytes alongside `removed` in `CleanupResponse` and surface "removed N (freed 12.4 MB)" in the UI cleanup toast
- Auto-refresh the routine LOGS view (or show a removed badge) after a CLEANUP NOW sweep so stale run output isn't shown for reaped workbenches
- Add the option to filter routines by repositories in the ui
