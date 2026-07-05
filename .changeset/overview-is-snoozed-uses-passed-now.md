---
"moadim": patch
---

fix(ui): derive the Overview page's per-source `snoozed` flag from the same `now` already threaded through its KPI/attention/upcoming-run math instead of sampling `js_sys::Date::now()` inline, so `is_snoozed`/`from_routine`/`sources_of` stay deterministic and host-testable (this was silently broken: `cargo test --workspace` panicked with "cannot call wasm-bindgen imported functions on non-wasm targets" in 4 `overview_tests`, invisible in CI because `test.yml` only runs bare `cargo test`, which skips the `ui` workspace member).
