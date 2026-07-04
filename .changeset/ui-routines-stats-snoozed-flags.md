---
"moadim": patch
---

feat(ui): add SNOOZED and FLAGS tiles to routines page stats bar

The Routines page stats bar previously only showed TOTAL, ENABLED,
DISABLED, DUE SOON, and UNREGISTERED AGENT. It now also shows:

- **SNOOZED** — count of routines with suppressed fires (clickable
  filter like DUE SOON; amber when non-zero)
- **FLAGS** — total open flags across all routines (red when non-zero)
- **DUE SOON** — now correctly excludes snoozed routines (same fix as
  the overview page in #945)

Adds `Snoozed` to `RoutineStatusFacet` with roundtrip codec support and
a filter-matching test.
