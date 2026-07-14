---
"moadim": patch
---

chore(lint): enable clippy::cast_lossless in the workspace

Adds `cast_lossless = "deny"` to both the root crate's and the `ui` crate's
`[lints.clippy]` tables, rejecting `as` casts that widen without loss (e.g.
`u32 as i64`) in favour of `From`/`Into`. An `as` cast stays silently legal
(and silently starts truncating) if the source or target type ever changes
size; `i64::from(x)` is the same widening but fails to compile the moment it
would no longer be lossless.

Fixed the single violation this surfaced, in `ui/src/routines/calendar.rs`'s
week-grid start calculation, replacing `... as i64` with
`i64::from(...)`. No behavior change.
