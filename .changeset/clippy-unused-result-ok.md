---
"moadim": patch
---

Enable `clippy::unused_result_ok`, which rejects `fallible_call().ok();` used as a statement purely to discard a `Result`. The trailing `.ok()` reads like a conversion rather than a decision, so a deliberately ignored failure is indistinguishable from one the author forgot to handle. The three sites it surfaced — the non-fatal `prebuilt.html` refresh in the client build script, and two temp-file cleanups in `service_log_tail_tests` — now use the explicit `let _ = ...;` form, with no behavior change.
