---
"moadim": patch
---

test: de-genericize the `with_home` test helper in `routine_storage_walk` and add a nested-call test so its `MOADIM_HOME_OVERRIDE` restore branch is exercised, restoring the repo's 100%-line-coverage gate (`cargo llvm-cov --fail-under-lines 100`) to green.
