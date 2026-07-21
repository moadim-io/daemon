---
"moadim": patch
---

Enable `clippy::inefficient_to_string` to reject calling `.to_string()` on a `&&T` (e.g. `&&str`) in favour of calling it on the dereferenced `&T` directly, avoiding an unnecessary extra indirection through `Display`/`ToString`. The codebase already had zero violations, so this is a lock-in with no behavior change.
