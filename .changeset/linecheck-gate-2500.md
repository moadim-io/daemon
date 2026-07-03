---
"moadim": patch
---

### Changed

- **Pre-push hook linecheck gate lowered from 3000 → 2500 lines per `.rs` file.** `service_tests.rs` (2677 lines) was split into `service_tests.rs` and `service_model_tests.rs` to satisfy the new ceiling.
