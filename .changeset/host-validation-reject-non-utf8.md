---
"moadim": patch
---

`host_validation` middleware: a present-but-non-UTF-8 `Host` or `Origin` header is now rejected with `403` instead of being silently treated the same as a missing header. `HeaderValue::to_str()` only rejects non-ASCII bytes, which no legitimate client ever sends in these headers, so falling through to "allow" on that error let an attacker bypass the DNS-rebinding/cross-origin allowlist entirely by sending garbage bytes in `Host`/`Origin`. Adds regression tests for both headers.
