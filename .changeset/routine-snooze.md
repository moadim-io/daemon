---
"moadim": minor
---

### Added

- **Routine snooze.** A new `snooze_routine` MCP tool lets an agent skip its own
  upcoming *scheduled* (cron) fires — either until an absolute unix timestamp
  (`snoozed_until`) or for a fixed count of upcoming fires (`skip_runs`) —
  without touching `enabled`, the crontab, or manual triggers. A snoozed fire
  is skipped before any workbench is spawned; `snoozed_until` clears itself
  once elapsed and `skip_runs` decrements to zero, at which point the routine
  fires normally again. Manual triggers (`trigger_routine`, the UI button)
  always bypass snooze. The Routines table shows a `SNOOZED` badge for
  affected routines.
