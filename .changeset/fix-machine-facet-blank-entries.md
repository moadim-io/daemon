---
"moadim": patch
---

fix(ui,client): omit blank machine entries from the Machine filter dropdown

`distinct_machines_r` (Yew UI) and `distinctMachines` (React client) collected every raw
`machines` string into the Machine facet's dropdown options, including blank/whitespace-only
entries. The API already rejects such entries on create/update (`validate_machines`, #600),
but routines written before that guard existed can still carry one, and `routineHealth`/
`routine_health` already treat it as "no real machine assigned" (dormant). Left unfiltered, a
legacy blank entry surfaced as a stray, unlabeled blank option in the dropdown, distinct from
"Any" and "Unassigned". Both helpers now skip blank/whitespace-only entries, matching the
health check's existing tolerance for this legacy data shape.
