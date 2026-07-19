---
"moadim": patch
---

Enable `clippy::panic` workspace-wide to forbid `panic!()` in production code, matching the existing `unwrap_used`/`expect_used` hardening against unhandled panics in the long-running daemon process. Test code stays exempt via `allow-panic-in-tests` in `clippy.toml`.
