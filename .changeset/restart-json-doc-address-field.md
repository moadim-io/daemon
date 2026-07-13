---
"moadim": patch
---

docs(cli): document the `address` field in `moadim restart --json`'s output shape

`restart_json` (`src/cli/restart.rs`) has emitted `{"old":…,"new":…,"address":…}` since the
`address` field was added, but both the function's own doc comment and the README's `restart`
row still documented the older two-field shape (`{"old":N|null,"new":M}`), which the function's
own test (`restart_json_reports_old_new_pid_and_address`) already contradicted. Updated both to
match the real output. No behavior change.
