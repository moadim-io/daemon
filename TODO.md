# TODOs

> _Today's TODO, tomorrow's changelog. Ship one, dream up two._

This is a list of todos for consumption, in a pr remove the todo you have implemented and add any new ones you think of.

- Add spell check for pre commit
- Add validation dialog before shutdown
- Surface and edit a routine's workbench TTL (`ttl_secs`) in the UI
- Add an endpoint/CLI to trigger workbench cleanup on demand (not only the hourly sweep)
- Add a day-detail popover to the routines calendar: clicking a day lists each fire time (HH:MM) with its routine, and a "run now" shortcut per routine
- Link the routines calendar UI to the new `GET /routines.ics` feed: add a "SUBSCRIBE" button that copies the feed URL, plus a per-routine `?routine=<id>` filter on the endpoint
- Fold the host's local timezone into the `.ics` feed as a `VTIMEZONE` block with `DTSTART;TZID=` local times, so subscribers see fire times in the routine's own zone instead of only UTC
- Auto-stamp the release version/date into CHANGELOG.md from the `chore(release)` step so the `## [Unreleased]` section rolls over on tag
- Add a CI check that fails a PR when it changes `src/` or `ui/` but leaves `## [Unreleased]` in CHANGELOG.md empty
- Have a commands folder for all the cli commands, we want to work with colocation of files
