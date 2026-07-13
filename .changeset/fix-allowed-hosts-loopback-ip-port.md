---
"moadim": patch
---

fix(security): add missing `127.0.0.1:<port>` entry to the loopback `Host`/`Origin` allowlist

`allowed_hosts()` added `localhost:<port>` and `[::1]:<port>` alongside the bare bind address
when the daemon's bind address carries a port, but never added the equivalent
`127.0.0.1:<port>` entry — even though the bare `127.0.0.1` (no port) was already allowed. A
browser sending `Host: 127.0.0.1:<port>` (the common case for anyone loading the UI via the
raw IPv4 loopback address instead of `localhost`) was silently rejected with 403 by the
DNS-rebinding guard from issue #266, while the functionally identical `localhost:<port>` was
let through.
