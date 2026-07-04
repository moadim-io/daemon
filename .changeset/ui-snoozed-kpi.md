---
"moadim": patch
---

feat(ui): add SNOOZED KPI tile to overview dashboard

Surface the count of enabled routines whose scheduled fires are
currently suppressed (snoozed or skip-runs active) as a SNOOZED tile
in the overview stat row. Amber when non-zero, green when clear —
makes it immediately visible when routines are intentionally silenced.
