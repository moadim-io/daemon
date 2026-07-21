---
"moadim": patch
---

Enable `clippy::float_cmp` to reject a direct `==`/`!=` comparison between floating-point values. Binary floating-point can't represent most decimal fractions exactly, so two values computed by equivalent-looking paths can differ in the last bit and silently fail an exact-equality check that was meant to test "close enough". The codebase is already clean, so `deny` locks that in. No behavior change.
