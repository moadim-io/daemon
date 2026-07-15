---
"moadim": patch
---

feat(ui): day-detail popover on the routines calendar

Clicking a day number in the routines calendar month view now opens a popover listing that
day's fire times (`HH:MM`) per routine, sorted chronologically, each with a "▶ RUN" button
that triggers the routine immediately via the existing `POST /api/v1/routines/{id}/trigger`
endpoint. Closes the TODO.md item asking for this. Frontend-only: new pure `fires_on_day`
(`ui/src/schedule.rs`) and `day_fire_rows` (`ui/src/routines/calendar.rs`) helpers, both
host-tested; no backend or API changes.
