---
"moadim": patch
---

Enable `clippy::manual_ok_or` to reject a `match`/`if let` that manually converts an `Option` to a `Result` (`Some(x) => Ok(x), None => Err(e)`) instead of `.ok_or(e)`. The codebase was already clean against this lint; `deny` locks that in.
