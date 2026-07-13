---
"moadim": patch
---

test(middlewares): cover `allowed_hosts` when `MOADIM_BIND_ADDR` has no port

`allowed_hosts()` splits the configured bind address on `:` to derive a port and add
`localhost:<port>`/`[::1]:<port>` entries to the `Host`/`Origin` allowlist. The branch where
`MOADIM_BIND_ADDR` has no port (e.g. an operator setting it to a bare `0.0.0.0`) was never
exercised by a test — one of several gaps keeping the repo's 100%-line-coverage gate (`cargo
llvm-cov --fail-under-lines 100`, run in the pre-push hook) below 100% on `main`. Adds a test
asserting the port-suffixed entries are skipped in that case, bringing this file to 100%. No
behavior change.
