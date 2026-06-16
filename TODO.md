# TODOs

> _Today's TODO, tomorrow's changelog. Ship one, dream up two._

This is a list of todos for consumption, in a pr remove the todo you have implemented and add any new ones you think of.

- Run the `typos` spell check in CI (GitHub Actions) so PRs are gated even without local hooks installed
- Add a `cargo xtask spellcheck` (or `make spell`) wrapper that installs and runs `typos` so contributors don't need to know the tool name
- Add an endpoint/CLI to trigger workbench cleanup on demand (not only the hourly sweep)
- Add a day-detail popover to the routines calendar: clicking a day lists each fire time (HH:MM) with its routine, and a "run now" shortcut per routine
- Add an iCal (`.ics`) export endpoint for routine schedules so upcoming fire times can be subscribed to in external calendars
- Auto-stamp the release version/date into CHANGELOG.md from the `chore(release)` step so the `## [Unreleased]` section rolls over on tag
- Add a CI check that fails a PR when it changes `src/` or `ui/` but leaves `## [Unreleased]` in CHANGELOG.md empty
- Have a commands folder for all the cli commands, we want to work with colocation of files
- Dismiss any open UI modal/dialog (edit, delete-confirm, shutdown-confirm) with the Esc key
- Show server uptime as a humanized duration (e.g. "2h 14m") next to the health badge in the header
- Have better daemon logging, with timestamps and log levels
- Add a TTL preset row (1h / 1d / 7d / 30d) under the WORKBENCH TTL input in the routine form, mirroring the cron schedule presets
- Show a humanized retention countdown ("expires in 2d" / "expired") per finished run in the routine LOGS view, derived from the run's finish time and the routine's effective TTL
