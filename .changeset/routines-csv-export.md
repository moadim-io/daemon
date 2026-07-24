---
"moadim": patch
---

feat(client): export the current Routines table view to CSV (#1404). Adds an `EXPORT CSV` toolbar action that downloads whatever rows the active search/facet filters, sort, and group-by have already narrowed down to — title, schedule, agent, machines, tags, status, next/last run timestamps, and flag count — using only fields the existing `GET /routines` response already returns.
