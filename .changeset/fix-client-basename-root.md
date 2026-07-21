---
"moadim": patch
---

Fix the web UI rendering a blank page at `GET /` (#1379). Removing the legacy Yew UI (v1.5.0) promoted the React client to the server root, but the client's `BrowserRouter` kept its old `basename="/client"`, so the router matched nothing at `/` and rendered an empty page. The router (and Vite `base`) now resolve from `/`, and old `/client` and `/client/*` links — including query strings like `/client/routines?history=<id>` — permanently redirect to their root-relative equivalents.
