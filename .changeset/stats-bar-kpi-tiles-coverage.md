---
"moadim": patch
---

test(client): cover `StatsBar`'s KPI tile counts and status-facet toggle

`client/src/pages/routines/StatsBar.tsx` — the KPI tile row above the routines table — had 0%
test coverage despite deriving eight non-trivial counts (total/enabled/disabled, due-soon,
snoozed, dormant, flagged, unregistered-agent) from the loaded routine list. Adds a test file
covering the derived counts, the `has-dormant`/`has-flags` conditional classes, the toggle-on/
toggle-off click behavior, and `aria-pressed` state. No production code changes.
