---
"moadim": minor
---

feat(routines): expose `next_run_at` on the routine API response

`GET /routines` (and single-routine reads) already surfaced `schedule`,
`schedule_description`, and `timezone`, but never the computed next fire
time — you had to mentally evaluate the cron expression, open the
CALENDAR view, or subscribe to the `.ics` feed to find out when a routine
runs next. `RoutineResponse` now includes `next_run_at` (Unix epoch
seconds, host-local-timezone crontab semantics), reusing the same
`croner` evaluation the `.ics` feed and TTL sweep already perform. It is
`null` when the routine is disabled, the daemon is globally locked, or
`schedule` is unparseable or has no upcoming fire (e.g. `@reboot`).
Closes #369.
