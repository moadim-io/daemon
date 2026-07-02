---
"moadim": patch
---

### Fixed

- **Routine list no longer shows stale data after a page reload.** The server now sends `Cache-Control: no-store` on all API responses that don't already carry a cache directive, preventing browsers from heuristically caching `GET /routines` responses and serving stale JSON on reload.
