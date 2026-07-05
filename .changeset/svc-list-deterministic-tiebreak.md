---
"moadim": patch
---

fix(routines): make svc_list deterministic for tied sort keys

Routines come off a `HashMap`, whose iteration order is unspecified, so a
listing sorted by a field with duplicate values (e.g. several routines
created in the same second) previously rendered in an arbitrary, run-to-run
order. `svc_list` now breaks ties on the stable routine id, and reverses the
whole comparison (not just the sorted vector) for descending order so the
tiebreak direction stays consistent.
