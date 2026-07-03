---
"moadim": patch
---

### Changed

- **Pre-push hook now rejects `.rs` files exceeding 3000 lines.** Step 6 of `.githooks/pre-push` runs `linecheck --max-lines 3000` over all `src/**/*.rs` files. `cargo install linecheck` is required. `service_tests.rs` (3068 lines) was split into `service_tests.rs` and `service_flag_tests.rs` to satisfy the new gate.
