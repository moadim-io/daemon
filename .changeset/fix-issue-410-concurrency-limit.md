---
"moadim": patch
---

fix: add global concurrency limit to the HTTP server

The Axum router had no cap on in-flight requests, so a burst of concurrent
requests or a few hung `crontab`/`tmux` calls could exhaust the runtime's
worker/blocking pool and leave even `GET /health` unreachable. A
`tower::limit::GlobalConcurrencyLimitLayer` (a single shared semaphore, cap
64) now sits as the outermost layer, queuing excess requests instead of
piling more work onto the runtime.
