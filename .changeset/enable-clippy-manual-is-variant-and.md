---
"moadim": patch
---

Enable `clippy::manual_is_variant_and` to reject a `match`/`if let` that manually re-derives what `Option::is_some_and`/`is_none_or` or `Result::is_ok_and`/`is_err_and` already compute (e.g. `match opt { Some(x) => pred(x), None => false }`) instead of calling the combinator directly. The codebase was already clean against this lint; `deny` locks that in.
