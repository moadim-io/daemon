---
"moadim": patch
---

### Added

- **Repository filter for the Routines table.** The REST `GET /routines`
  endpoint has supported a `?repository=` filter for a while, but the web UI
  had no way to use it — the only client-side facets were status, agent, and
  machine. Added a REPOSITORY dropdown to the Routines filter bar (mirroring
  the existing agent/machine facet pattern), populated from the distinct
  repository URLs across loaded routines, so operators can narrow a dense
  routines list to a single repo without hand-editing the query string.
