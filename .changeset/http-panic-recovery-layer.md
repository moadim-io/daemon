---
"moadim": patch
---

### Fixed

- **A panicking HTTP handler no longer resets the connection with no response.**
  Added `tower_http::catch_panic::CatchPanicLayer` as the outermost layer of
  the Axum router, so an unexpected panic inside a handler now yields a plain
  `500 Internal Server Error` response instead of the client seeing a dropped
  connection and the server logging nothing (issue #337).
