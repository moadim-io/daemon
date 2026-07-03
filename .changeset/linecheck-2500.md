---
"moadim": patch
---

### Changed

- **Pre-push linecheck gate lowered from 3000 → 2500 lines per `.rs` file.** `service_tests.rs` was split again to comply — tags, machines, and model tests now live in `service_model_tests.rs`.
