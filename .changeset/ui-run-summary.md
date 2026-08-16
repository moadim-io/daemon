---
"client": patch
---

Surface the agent-authored `summary.md` for a run in the UI (Run Detail page and a routine's History panel), above the raw log — the same "headline result before full output" pattern CI tools like GitHub Actions job summaries use. The daemon already writes this file per run and serves it at `GET /routines/{id}/runs/{workbench}/summary`, but no page ever rendered it.
