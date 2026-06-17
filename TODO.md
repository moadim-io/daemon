# TODOs

> _Today's TODO, tomorrow's changelog. Ship one, dream up two._

This is a list of todos for consumption, in a pr remove the todo you have implemented and add any new ones you think of.

- Add a `cargo xtask spellcheck` (or `make spell`) wrapper that installs and runs `typos` so contributors don't need to know the tool name
- Add a `test + coverage` CI job that mirrors the pre-push hook's final gate (`cargo llvm-cov --fail-under-lines 100 --ignore-filename-regex 'src/main\.rs'`) so the 100% coverage contract is enforced in PRs, not just locally
- Add a `concurrency` group to `lint.yml` (group by workflow + PR ref, `cancel-in-progress: true`) so a new push to a PR cancels the superseded lint run instead of piling up redundant jobs
- Add a CI step (or weekly scheduled job) that checks whether a newer `crate-ci/typos` release exists than the pinned tag and opens/updates a tracking issue, so the pin gets bumped on a cadence instead of drifting silently
- Pin the remaining third-party GitHub Actions (e.g. `actions/checkout@v5`) to full commit SHAs, mirroring the typos pin, so every CI dependency is reproducible
- Add a day-detail popover to the routines calendar: clicking a day lists each fire time (HH:MM) with its routine, and a "run now" shortcut per routine
- Link the routines calendar UI to the new `GET /routines.ics` feed: add a "SUBSCRIBE" button that copies the feed URL, plus a per-routine `?routine=<id>` filter on the endpoint
- Fold the host's local timezone into the `.ics` feed as a `VTIMEZONE` block with `DTSTART;TZID=` local times, so subscribers see fire times in the routine's own zone instead of only UTC
- Auto-stamp the release version/date into CHANGELOG.md from the `chore(release)` step so the `## [Unreleased]` section rolls over on tag
- Add a CI check that the topmost `## [x.y.z]` heading in CHANGELOG.md matches `Cargo.toml`'s version on tag pushes, so a release tag can't ship with a stale changelog version
- Have a commands folder for all the cli commands, we want to work with colocation of files
- Dismiss any open UI modal/dialog (edit, delete-confirm, shutdown-confirm) with the Esc key
- Show server uptime as a humanized duration (e.g. "2h 14m") next to the health badge in the header
- Have better daemon logging, with timestamps and log levels
- Add a TTL preset row (1h / 1d / 7d / 30d) under the WORKBENCH TTL input in the routine form, mirroring the cron schedule presets
- Show a humanized retention countdown ("expires in 2d" / "expired") per finished run in the routine LOGS view, derived from the run's finish time and the routine's effective TTL
- Enrich `moadim status --json` with the server's liveness details from `GET /health` (e.g. `uptime_secs`) so a single call returns running-state + age, not just the local PID
- Give `moadim stop` the same script-friendly exit-code contract as `status`/`cleanup` (exit 3 when no server was running to stop, 0 when a running server was asked to shut down) and document it in the README CLI table
- Add a `moadim status --wait[=SECS]` flag that polls `GET /health` until the server is reachable (or the timeout elapses), exiting 0 on success and the existing exit-3 on timeout, so scripts can block on startup instead of sleeping
- Add a `moadim restart --interactive` (or `-i`) flavor that restarts the daemon in the foreground attached to the terminal instead of detached, mirroring `moadim -i`
- Have `moadim restart` emit its PID-rotation summary as a `--json` object (`{"old":N|null,"new":M}`) too, mirroring the `status`/`cleanup` `--json` contract
- Give `moadim restart` a `--quiet`/`-q` flag that prints only the rotation line (`restarted: pid <old> -> <new>`) and suppresses the UI/stop/logs hint block, for script consumption
- Return the freed disk bytes alongside `removed` in `CleanupResponse` and surface "removed N (freed 12.4 MB)" in the UI cleanup toast
- Auto-refresh the routine LOGS view (or show a removed badge) after a CLEANUP NOW sweep so stale run output isn't shown for reaped workbenches
- Add the option to filter routines by repositories in the ui
