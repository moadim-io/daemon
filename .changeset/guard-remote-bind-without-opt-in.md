---
"moadim": patch
---

fix: refuse to start on an unauthenticated non-loopback bind (`MOADIM_BIND_ADDR`) unless `MOADIM_ALLOW_REMOTE=1` is explicitly set. Closes #253.
