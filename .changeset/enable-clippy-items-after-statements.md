---
"moadim": patch
---

chore(lint): enable `clippy::items_after_statements`

Fixes the 3 existing violations — a `use` mid-function in `read_log_tail_of_len` (`src/routines/service_log_tail.rs`), a `use` mid-test in `service_overlap_guard_tests.rs`, and a `const` mid-test in `routines_sync_tests.rs` — by hoisting each item to the top of its block. Enables `items_after_statements = "deny"` to lock in the zero-violation state going forward. No behavior change.
