---
"moadim": patch
---

chore(routines): remove duplicate tests left behind in `service_tests.rs`

`service.rs`'s test suite was split into focused sibling files (`service_sync_tests.rs`,
`service_slug_tests.rs`, `service_trigger_tests.rs`, `service_logs_tests.rs`,
`service_model_tests.rs`, `service_coverage_tests.rs`, ...) over time, but the original
`service_tests.rs` was never pruned of the tests that moved — 46 of its 71 tests were
byte-for-byte duplicates already covered by a sibling file, running twice on every `cargo
test` for zero extra coverage. This also pushed `service_tests.rs` to 2120 lines, well past
the repo's 700-line pre-push gate (`.githooks/pre-push`'s `linecheck` step), which currently
fails on plain `main` for anyone who has the git hooks installed per CONTRIBUTING.md.

Removes the 46 duplicate tests, moves the 2 remaining tests that shared a now-file-local
helper into a new `service_update_not_found_tests.rs`, and leaves `service_tests.rs` at 661
lines (once again under the gate). `cargo test` still passes with the same net coverage
(886 vs the prior 932 — the difference is exactly the 46 duplicates removed), confirmed via
`cargo llvm-cov --fail-under-lines 100`.
