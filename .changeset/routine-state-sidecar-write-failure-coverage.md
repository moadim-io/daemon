---
"moadim": patch
---

Add a test for `write_routine` returning an error when `state.local.toml`'s path is occupied by a directory, closing the last untested error branch in `write_runtime_state` (`routine_storage.rs`). No behavior change — test-only.
