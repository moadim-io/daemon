---
"moadim": patch
---

chore: split every remaining file over the 500-line pre-push gate

Follow-up to #941/#1014/#1017. Splits the rest of the backlog left after
#1017 (which got 14 files under 600, most already under 500) — every
`.rs` file in the repo is now ≤500 lines, satisfying `.githooks/pre-push`'s
`linecheck --max-lines 500` gate with no exceptions left. All splits are
pure code moves (functions/tests relocated verbatim into new sibling
modules) with no behavior change:

- `src/routines/service.rs` family (`service_sync_tests.rs`,
  `service_trigger.rs`, `service_tests.rs`, `service_flag_tests.rs`,
  `service_slug_tests.rs`, `service_coverage_tests.rs`) → new
  `service_log_tail.rs`, `service_field_validation_tests.rs`,
  `service_list_tests.rs`, `service_rename_machine_tests.rs`,
  `service_update_apply_tests.rs`, `service_prompt_tests.rs`
- `src/routine_storage.rs` family (`routine_storage_tests.rs`,
  `routine_storage_migration_tests.rs`, `routine_storage_snooze_tests.rs`)
  → new `routine_storage_prompt_sidecar_tests.rs`,
  `routine_storage_prompt_file_migration_tests.rs`,
  `routine_storage_trigger_log_migration_tests.rs`
- `src/routes/http.rs` → new `src/routes/http_listener.rs`
- `src/routes/mcp_tests.rs` → new `src/routes/mcp_parity_tests.rs`
- `src/cli.rs` / `cli_spawn_tests.rs` → new `src/cli_spawn_error_tests.rs`
- `src/routines/command.rs` → new `src/routines/command_path_resolution.rs`
- `src/routines/cleanup/cleanup_tests.rs` → new
  `cleanup_run_history_tests.rs`
- `src/sync/mod_tests.rs` → new `src/sync/mod_replace_block_tests.rs`
- `src/commands.rs` → new `src/commands_http.rs`
- `ui/src/routines/page.rs` → new `ui/src/routines/actions.rs`
- `ui/src/routines/filter.rs` / `filter_tests.rs` → new
  `filter_distinct.rs`, `filter_distinct_tests.rs`
- `ui/src/routines/state_tests.rs` → new `state_group_by_tests.rs`
- `ui/src/main.rs`, `overview.rs`, `command_palette.rs`,
  `schedule_heatmap.rs` → new `ui/src/health.rs`, `cron_utils.rs`,
  `overview_stats.rs`, `command_palette_match.rs`, `schedule_heatmap_grid.rs`

`cargo test --workspace` (912 + 259 passed), `cargo clippy --workspace
--all-targets -- -D warnings`, `cargo llvm-cov --fail-under-lines 100`, and
`cargo doc` (deny warnings, including broken intra-doc links) all pass.
`linecheck --max-lines 500` across every `.rs` file in `src/` and `ui/src/`
now exits clean with zero violations.
