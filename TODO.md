# TODOs

> _Today's TODO, tomorrow's changelog. Ship one, dream up two._

This is a list of todos for consumption, in a pr remove the todo you have implemented and add any new ones you think of.

- Add a way to see all the routines in the UI as a calendar view
- Add spell check for pre commit
- Add validation dialog before shutdown
- Surface and edit a routine's workbench TTL (`ttl_secs`) in the UI
- Add an endpoint/CLI to trigger workbench cleanup on demand (not only the hourly sweep)
- Auto-stamp the release version/date into CHANGELOG.md from the `chore(release)` step so the `## [Unreleased]` section rolls over on tag
- Add a CI check that fails a PR when it changes `src/` or `ui/` but leaves `## [Unreleased]` in CHANGELOG.md empty
- Have a commands folder for all the cli commands, we want to work with colocation of files
