---
"moadim": patch
---

Add a test for `svc_set_power_saving` returning 500/Internal when `write_routine` fails (read-only config dir), closing the last untested error branch in that handler. No behavior change — test-only.
