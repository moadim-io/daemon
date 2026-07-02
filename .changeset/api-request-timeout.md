---
"moadim": patch
---

### Added

- **A 30s per-request deadline on the REST API (`/api/v1/**`).** Previously the router had no request timeout at all: a wedged handler (e.g. blocking `crontab`/`tmux`/filesystem I/O with no `spawn_blocking`, #360) could hold its connection and a Tokio worker open forever with no upper bound and no error response. `POST`/`GET`/etc. requests to `/api/v1/**` that exceed the deadline now abort with `408 Request Timeout` instead of hanging indefinitely. The long-lived `/mcp` SSE stream is deliberately left outside this layer so legitimate streaming connections are unaffected (#402).
