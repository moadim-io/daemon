---
"moadim": patch
---

fix(ui,client): align the Routines "Dormant" status filter with the health badge/KPI definition of dormant. Both treated an empty `machines` list as dormant, but only the health badge/KPI (not the filter facet) also treated a list holding only blank/whitespace entries as dormant — so a routine could show a "DORMANT" badge and count toward the dormant KPI while filtering by `Status: Dormant` hid it. The filter now uses the same "no real machine assigned" check as the health/KPI logic in both the Yew UI and the React client.
