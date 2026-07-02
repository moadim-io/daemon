---
"moadim": patch
---

### Added

- **Per-routine power-saving mode.** A routine can now be paused for power
  saving independently of its `enabled` toggle — `enabled` stays user-owned
  intent, `power_saving` is a separate, system/policy-owned throttle that both
  must clear for a firing to launch (`enabled && !power_saving`). Set/cleared
  via the new `set_power_saving` MCP tool (`svc_set_power_saving`); persisted
  in the gitignored `state.local.toml` sidecar like `snoozed_until`/`skip_runs`,
  never in the tracked `routine.toml`, and never touched by create/update. Both
  `trigger_routine` and the routine's cron schedule now refuse to launch while
  it (or `enabled: false`) is active, with a distinct message naming which one.
  The web UI's health badge and "Run now" tooltip distinguish `POWER SAVING`
  from `DISABLED`.
