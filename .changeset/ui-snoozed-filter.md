---
"moadim": patch
---

fix(ui): exclude snoozed routines from DUE SOON count and UPCOMING RUNS table

Snoozed routines appeared in the overview's DUE SOON KPI and UPCOMING
RUNS table as if they would fire, even though their scheduled fires are
suppressed. Fixes both to only include enabled, non-snoozed sources so
the dashboard reflects what will actually run.
