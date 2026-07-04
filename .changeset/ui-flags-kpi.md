---
"moadim": patch
---

feat(ui): add FLAGS KPI tile to overview dashboard

Surface the total count of open flags across all routines as a FLAGS
tile in the overview stat row. Red when non-zero, green when clear —
gives operators an at-a-glance signal without navigating into individual
routines. Adds `flag_count` to `SchedSource` and `flags` to `Kpis`.
