---
"moadim": patch
---

Add a Reliability page to the React client, ranking routines by success rate, failure streak, and flakiness, with per-routine p50/p95 run duration and a slower-trend regression flag. Frontend-only — reads the existing `GET /routines/runs` payload.
