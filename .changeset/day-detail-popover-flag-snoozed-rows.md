---
"moadim": patch
---

fix(ui): flag snoozed routines in the routines calendar's day-detail popover

The month grid already dims a routine's chip (amber, reduced opacity) when it is snoozed
(`snoozed_until` in the future, or `skip_runs` still pending), since that fire will be
silently skipped rather than actually run. The day-detail popover added alongside it listed
every enabled routine's fire time the same way regardless of snooze state, so a user opening
the popover lost that signal and could believe a snoozed routine's listed time will fire.
`day_fire_rows` now also reports each row's snoozed status (reusing the existing
`is_routine_snoozed` helper) and the popover renders a "SNOOZED" badge on those rows, matching
the styling and wording already used elsewhere (the routines table's health badge).
