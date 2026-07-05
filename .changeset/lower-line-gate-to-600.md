---
"moadim": patch
---

chore: lower the pre-push line-count gate from 700 to 600 lines

Tightens `.githooks/pre-push`'s `linecheck` step to 600 lines per `.rs` file
(down from 700) to keep files smaller and easier to review. Splits every
file that grew past the new gate, moving cohesive chunks into new sibling
modules with no behavior or test-body changes:

- `src/routine_storage.rs` / `routine_storage_tests.rs` → new
  `routine_storage_migrations.rs` and `routine_storage_sidecar_state_tests.rs`
- `src/cli.rs` / `cli_tests.rs` → new `cli_query.rs` and `cli_query_tests.rs`
- `src/routines/service.rs` family (`service_tests.rs`,
  `service_trigger_tests.rs`, `service_trigger.rs`) → new
  `service_ceiling_tests.rs`, `service_snooze_tests.rs`, `service_ansi_tests.rs`,
  and `service_trigger_flags.rs`
- `src/routes/http.rs` family (`http_tests.rs`, `http_listener_tests.rs`) → new
  `http_settings_routes.rs`, `http_settings_routes_tests.rs`, and
  `http_listener_lock_tests.rs`
- `ui/src/routines/page.rs` → new `ui/src/routines/bulk_actions.rs`
- `ui/src/routines/filter_tests.rs` → new
  `ui/src/routines/filter_facet_codec_tests.rs`
- `ui/src/main.rs` / `overview.rs` → new `ui/src/header.rs` and
  `ui/src/overview_attention.rs`

`cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`,
`cargo llvm-cov --fail-under-lines 100`, and the full pre-push gate all pass.
