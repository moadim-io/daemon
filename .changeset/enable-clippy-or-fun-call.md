---
"moadim": patch
---

chore(lint): enable clippy::or_fun_call in the root crate

Adds `or_fun_call = "deny"` to the root crate's `[lints.clippy]` table. It rejects a function
call passed directly as the fallback argument to `unwrap_or`/`ok_or`/`and`/`or`-style methods
(e.g. `opt.unwrap_or(expensive())`) in favour of the lazy `_else` form
(`opt.unwrap_or_else(expensive)`) — the eager form always evaluates the fallback, even on the
common path where the value is already present, doing needless work (or a needless allocation)
on every call.

The codebase is already clean under it (zero violations), so `deny` locks that in. No behavior
change.
