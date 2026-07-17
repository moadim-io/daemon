---
"moadim": patch
---

Split `src/cli/mod.rs`, `src/cli/tests.rs`, and `src/routines/ical_tests.rs` — all three had
grown past the 500-line `linecheck` gate (issue #974), which was failing on `main`. Extracted
the bind-address/loopback-policy logic into `src/cli/bind.rs` (with its tests in
`src/cli/bind_tests.rs`), and the `svc_ical`/`svc_ical_routine`/`build_ical` service-layer tests
into `src/routines/ical_service_tests.rs`. No behavior change.
