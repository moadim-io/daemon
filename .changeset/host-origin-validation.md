---
"moadim": patch
---

Reject requests with a disallowed `Host` header, and state-changing requests with a cross-origin `Origin` header, closing the DNS-rebinding / browser cross-origin gap against the unauthenticated loopback API (#266). Extend the allowlist for reverse-proxy deployments with `MOADIM_ALLOWED_HOSTS`.
