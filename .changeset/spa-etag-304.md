---
"moadim": minor
---

### Added

- **`ETag` + `304 Not Modified` for the web UI.** `GET /` (and the SPA fallback
  for client-routed paths) now sends a strong `ETag` for the embedded ~1.1 MB
  `index.html`, and honors a matching `If-None-Match` with a bodyless `304`
  instead of re-sending the full body on every load/refresh. `Cache-Control:
  no-cache` keeps the browser revalidating on each request rather than trusting
  a local TTL, since the content can change on any daemon upgrade. (#401)
