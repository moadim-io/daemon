---
"moadim": patch
---

chore: split 7 file groups that had grown past the new 500-line pre-push gate

Follow-up to #941/#1014, which already lowered `.githooks/pre-push`'s
`linecheck` step from 700 to 500 (config-only, no splits). Splits the
biggest offenders from that PR's backlog into new sibling modules, moving
cohesive chunks with no behavior or test-body changes:

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

These 14 files are all now under 600 lines; most are under 500. A handful
still land in the 500-590 range and, along with the rest of #1014's
original backlog, are left for further follow-up splits — same
config-only-then-split-incrementally approach #1014 itself took.

`cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`,
and `cargo llvm-cov --fail-under-lines 100` all pass.
