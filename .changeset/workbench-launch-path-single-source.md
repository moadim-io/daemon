---
"moadim": patch
---

### Fixed

- **Workbench launch path now derived from `paths::workbenches_dir()`.** The
  generated cron launch command hardcoded `WB="$HOME/.moadim/workbenches/$SLUG-$TS"`
  instead of going through the same seam the reaper (`routines/cleanup/mod.rs`)
  and the LOGS view (`routines/service.rs`) already use. With
  `MOADIM_HOME_OVERRIDE` set, this meant a run was *launched* under one path but
  *reaped and listed* under another — leaking workbenches the reaper never sees
  and leaving the LOGS view empty for real runs. The launch command now resolves
  its base through `paths::workbenches_dir()`, with a regression test asserting
  the two stay in sync under the override. No behavior change for the default
  install. (#601)
