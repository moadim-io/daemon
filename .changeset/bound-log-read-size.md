---
"moadim": patch
---

fix(routines): cap agent.log reads to the last 2 MiB

`GET /routines/{id}/logs`, the run-detail log endpoint, and the `logs` MCP
tool all read a run's `agent.log` in full via `read_to_string`. A
long-running or noisy agent can grow that file without bound, so serving it
whole risks an out-of-memory daemon and a multi-hundred-MB HTTP response for
one request. Both now go through a shared `read_log_tail` helper that caps
the read to the most recent 2 MiB, snapped to a UTF-8 character boundary so
a multi-byte character split by the byte-offset seek isn't mangled, and
prefixes a marker noting how many bytes were omitted.
