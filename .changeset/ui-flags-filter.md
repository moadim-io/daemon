---
"moadim": patch
---

feat(ui): make FLAGS tile in stats bar a clickable filter

The FLAGS tile in the routines stats bar was informational-only. It is
now a clickable filter button (like SNOOZED, DUE SOON, etc.) that narrows
the table to routines with one or more open flags. Clicking it again
clears the filter. The tile border and value turn red when any flags are
present.

Adds `RoutineStatusFacet::HasFlags` with codec roundtrip and one new host
test.
