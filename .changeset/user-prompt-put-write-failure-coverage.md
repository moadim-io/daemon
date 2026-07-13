---
"moadim": patch
---

Add a test for `PUT /config/user-prompt` returning 500 when the write itself fails (target path is a directory), closing the last untested error branch in that handler. No behavior change — test-only.
