# TODOs

> _Today's TODO, tomorrow's changelog. Ship one, dream up two._

This is a list of todos for consumption, in a pr remove the todo you have implemented and add any new ones you think of.

- Add a way to see all the routines in the UI as a calendar view
- Add spell check for pre commit
- Add validation dialog before shutdown
- Surface and edit a routine's workbench TTL (`ttl_secs`) in the UI
- Add change log
- Add a "CLEANUP NOW" button in the UI that calls `POST /routines/cleanup` and shows the removed count
- Add a `moadim cleanup` CLI subcommand that POSTs `/routines/cleanup` to a running daemon and prints how many workbenches were reaped
