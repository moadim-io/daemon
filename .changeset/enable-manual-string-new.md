---
"moadim": patch
---

chore(lint): enable `clippy::manual_string_new`

Requires `String::new()` over `"".into()`/`"".to_string()` for constructing
an empty `String`. Fixed the two violations this newly-`deny`d lint caught
(both in test fixtures).
