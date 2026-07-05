---
"moadim": patch
---

Enable `clippy::needless_pass_by_ref_mut` in `[lints.clippy]`. The codebase was already clean against it (no violations), so this is a lint-only change that locks in the invariant that every `&mut` parameter is actually mutated through.
