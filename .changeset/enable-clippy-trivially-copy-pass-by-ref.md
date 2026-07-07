---
"moadim": patch
---

Enable `clippy::trivially_copy_pass_by_ref` (deny) to reject `&T` parameters where `T` is a small `Copy` type — pass-by-value is at least as cheap and states the callee doesn't need the caller's own reference. Codebase was already compliant, no code changes needed.
