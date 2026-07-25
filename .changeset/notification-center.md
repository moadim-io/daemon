---
"moadim": patch
---

feat(client): persistent notification center — a bell icon in the header with an unread-count badge and a dropdown inbox of recent fleet-wide run failures, always polling regardless of which page is open and persisted via localStorage so it survives navigation/reload. Frontend-only, built entirely on `GET /routines/runs`, which the client already fetches elsewhere.
