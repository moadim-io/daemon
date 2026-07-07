---
"moadim": patch
---

chore(lint): enable `clippy::single_match_else`

Fixes the 2 existing violations — `machine::run`'s `Some("set")` arm and `defaults::ensure_default_routines`'s existing-routine lookup — each a `match` destructuring a single pattern with the rest falling to a catch-all arm. Switches both to `if let ... else`, and enables `single_match_else = "deny"` to lock in the zero-violation state going forward. No behavior change.
