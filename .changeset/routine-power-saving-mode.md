---
"moadim": minor
---

feat(routines): per-routine power-saving mode, orthogonal to enabled

Adds `power_saving: bool` alongside the existing user-owned `enabled` toggle:
both must hold (`enabled && !power_saving`) for a manual or scheduled trigger
to launch. `power_saving` is system/policy-owned, never touched by
create/update, and persisted in the gitignored `state.local.toml` sidecar like
`snoozed_until`/`skip_runs` rather than the tracked `routine.toml`. Set/cleared
via the new `set_power_saving` MCP tool. The web UI's health badge and
"Run now" tooltip distinguish `POWER SAVING` from `DISABLED`.
