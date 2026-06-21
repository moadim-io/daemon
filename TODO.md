# TODOs

> _Today's TODO, tomorrow's changelog. Ship one, dream up two._

This is a list of todos for consumption, in a pr remove the todo you have implemented and add any new ones you think of.

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
- Have `moadim cleanup --json` include the bound `address` field too (`{"running":bool,"removed":N,"address":…}`), so every `--json` command surfaces the endpoint it talked to, matching `status`/`stop`
- Add a `cli_tests` assertion that `status --json` and `stop --json` produce the SAME set of object keys, so the two shapes can't silently drift apart again as fields are added
- Add a CLI integration test (spawn a real listener on an ephemeral port, point the probe at it) that exercises the `status`/`cleanup`/`stop` network paths end-to-end, lifting `cli.rs` off its ~27% coverage floor toward the repo's 100% line-coverage gate
- Add a `moadim status --wait[=SECS]` flag that polls `GET /health` until the server is reachable (or the timeout elapses), exiting 0 on success and the existing exit-3 on timeout, so scripts can block on startup instead of sleeping
- Add a `moadim restart --interactive` (or `-i`) flavor that restarts the daemon in the foreground attached to the terminal instead of detached, mirroring `moadim -i`
- Have `moadim restart` emit its PID-rotation summary as a `--json` object (`{"old":N|null,"new":M}`) too, mirroring the `status`/`cleanup` `--json` contract
- Give `moadim restart` a `--quiet`/`-q` flag that prints only the rotation line (`restarted: pid <old> -> <new>`) and suppresses the UI/stop/logs hint block, for script consumption
- Return the freed disk bytes alongside `removed` in `CleanupResponse` and surface "removed N (freed 12.4 MB)" in the UI cleanup toast
- Auto-refresh the routine LOGS view (or show a removed badge) after a CLEANUP NOW sweep so stale run output isn't shown for reaped workbenches
- Add the option to filter routines by repositories in the ui
- Run the README's fenced `sh`/`json` blocks through a docs lint in CI (e.g. a `markdownlint`/`mdsh` check or a `--json | jq -e` smoke test) so the documented JSON shapes can't drift from the actual CLI output silently
