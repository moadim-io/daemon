# TODOs

> _Today's TODO, tomorrow's changelog. Ship one, dream up two._

This is a list of todos for consumption, in a pr remove the todo you have implemented and add any new ones you think of.

- Add spell check for pre commit
- Add validation dialog before shutdown
- Surface and edit a routine's workbench TTL (`ttl_secs`) in the UI
- Add an endpoint/CLI to trigger workbench cleanup on demand (not only the hourly sweep)
- Add a day-detail popover to the routines calendar: clicking a day lists each fire time (HH:MM) with its routine, and a "run now" shortcut per routine
- Add an iCal (`.ics`) export endpoint for routine schedules so upcoming fire times can be subscribed to in external calendars
- Auto-stamp the release version/date into CHANGELOG.md from the `chore(release)` step so the `## [Unreleased]` section rolls over on tag
- Add a CI check that fails a PR when it changes `src/` or `ui/` but leaves `## [Unreleased]` in CHANGELOG.md empty
- Have a commands folder for all the cli commands, we want to work with colocation of files
