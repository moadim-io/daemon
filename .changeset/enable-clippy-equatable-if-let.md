---
"moadim": patch
---

Enable `clippy::equatable_if_let`, rejecting an `if let PAT = expr` that only tests a single unit-like variant with no bindings extracted in favor of a direct `==` comparison. No behavior change — the codebase is already clean, so `deny` locks that in.
