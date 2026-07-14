---
"moadim": patch
---

Add a test for `cap_agent_log_to` propagating the `OpenOptions::open` error when the target path is a directory, closing an untested error branch in the watchdog's `agent.log` size cap. No behavior change — test-only.
