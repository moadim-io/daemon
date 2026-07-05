---
"moadim": patch
---

feat(ui): expose Snoozed, Flagged, and Agent-unregistered options in the STATUS filter dropdown

Three status facets (`Snoozed`, `HasFlags`, `AgentUnregistered`) were fully
implemented in the filter logic but had no corresponding option in the STATUS
drop-down, making them invisible to users. Operators can now select:

- **Snoozed** — routines whose scheduled fires are currently suppressed
- **Flagged** — routines with one or more open flags needing review
- **Agent unregistered** — routines whose agent config is missing
