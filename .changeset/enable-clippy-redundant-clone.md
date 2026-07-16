---
"moadim": patch
---

chore(lint): enable `clippy::redundant_clone` workspace-wide. Fixes 31 violations across the `ui`
crate and the root crate's test suite: intermediate `let x = x.clone();` shadows built for a `move`
closure that turned out to be `x`'s last use, and a few `field.clone()` reads passed straight into
a constructor that never touched the original value again. Each is replaced with a direct move of
the original. No behavior change.
