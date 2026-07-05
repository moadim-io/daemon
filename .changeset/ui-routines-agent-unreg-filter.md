---
"moadim": patch
---

feat(ui): make UNREGISTERED AGENT stat tile a clickable filter

The "UNREGISTERED AGENT" tile on the routines stats bar was a read-only
display div. It is now a clickable filter button (like DORMANT, FLAGS,
SNOOZED) that filters the table to show only routines whose agent is not
registered. The tile turns amber when any unregistered-agent routines exist.
A new `AgentUnregistered` variant is added to `RoutineStatusFacet` so
the filter state persists in the URL via the existing `status=` query param.
