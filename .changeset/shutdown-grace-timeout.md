---
"moadim": patch
---

fix(server): bound graceful-shutdown drain so `moadim stop` can't hang forever

`axum`'s graceful shutdown waits for every in-flight connection to close
before returning, so a long-lived stream (an `/mcp` SSE subscription, a slow
client) could keep that future pending indefinitely, hanging `moadim
stop`/`POST /shutdown` forever (#342). The server now caps the post-shutdown
drain to a bounded grace window (10s by default, overridable via
`MOADIM_SHUTDOWN_GRACE_MS` for tests) and forces a clean exit once it
elapses, logging a warning if connections were still open.
