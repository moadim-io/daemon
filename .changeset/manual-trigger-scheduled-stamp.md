---
"moadim": patch
---

fix(routines): manual triggers no longer clobber `last_scheduled_trigger_at`

A manual ("run now") routine trigger no longer overwrites `last_scheduled_trigger_at`. The launch script it shares with the scheduled path unconditionally appended the fire time to the routine's `scheduled.log`, so every manual run masqueraded as a scheduled fire and clobbered the real last-scheduled time. `build_routine_command` now takes a `TriggerSource` (`Scheduled`/`Manual`); only a genuine scheduled fire appends to `scheduled.log`, while a manual trigger launches the agent but leaves it untouched, staying tracked solely via `last_manual_trigger_at` (#478).
