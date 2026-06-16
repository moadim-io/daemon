# TODOs

> _Today's TODO, tomorrow's changelog. Ship one, dream up two._

This is a list of todos for consumption, in a pr remove the todo you have implemented and add any new ones you think of.

- Add a way to see all the routines in the UI as a calendar view
- Add validation dialog before shutdown
- Run the `typos` spell check in CI (GitHub Actions) so PRs are gated even without local hooks installed
- Add a `cargo xtask spellcheck` (or `make spell`) wrapper that installs and runs `typos` so contributors don't need to know the tool name
- Surface and edit a routine's workbench TTL (`ttl_secs`) in the UI
- Add an endpoint/CLI to trigger workbench cleanup on demand (not only the hourly sweep)
- Add change log
