---
"moadim": patch
---

Enable `clippy::implicit_clone` to reject an indirect `.to_string()`/`.to_owned()`-style clone of a value that's already the target type, in favour of calling `.clone()` directly. The codebase already had zero violations, so this is a lock-in with no behavior change.
