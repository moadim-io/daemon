---
"moadim": patch
---

fix(routines): iCalendar feed now skips power-saving and snoozed fires

`GET /routines.ics` only excluded disabled routines and unparseable schedules,
so a routine in power-saving mode, snoozed via `snoozed_until`, or with
`skip_runs` pending still advertised upcoming fire times that
`svc_trigger_scheduled` would actually refuse to spawn — a subscribed calendar
lied about what would run. The feed now filters/skips those fires the same way
the trigger path does, so `.ics` subscribers never see a run that will
silently no-op.
