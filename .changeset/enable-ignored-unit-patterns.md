---
"moadim": patch
---

chore(lint): enable `clippy::ignored_unit_patterns` in the root crate. Rewrites the four
`tokio::select!` arms in `src/routes/http_listener.rs` that matched a `()`-typed future with `_`
to match `()` explicitly instead, so the pattern states its type rather than leaving the reader to
confirm `_` isn't silently discarding something meaningful. No behavior change.
