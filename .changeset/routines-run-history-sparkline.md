---
"moadim": minor
---

### Added

feat(ui): inline run-history sparkline column in the Routines table

Each row now shows a compact strip of ticks for its last ~10 runs (green =
success, red = failed, pulsing amber = running, gray = unknown/no data),
between LAST FIRE and AGENT — an at-a-glance pass/fail trend without opening
the routine's HISTORY page, mirroring the "pipeline graph" pattern common to
CI dashboards. Reuses the existing fleet-wide `GET /routines/runs` endpoint
(already backing the Overview page's recent-runs panel), fetched once and
grouped client-side by routine — no new API calls per row (#1103).
