---
"moadim": patch
---

chore(lint): enable `clippy::unreadable_literal`

Locks in the codebase's existing (near-)zero-violation state for `clippy::unreadable_literal`, so a future long integer literal without `_` digit-group separators fails CI instead of shipping a number that's hard to judge the magnitude of at a glance. Fixes the one existing violation (`424242` → `424_242` in `restart_tests.rs`). No behavior change.
