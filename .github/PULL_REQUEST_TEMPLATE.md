<!--
Thanks for contributing to moadim! Keep PRs small and focused — one logical
change per PR. Fill in the sections below; delete any that don't apply.
-->

## What & why

<!-- What does this change do, and what problem does it solve? -->

Fixes #<!-- issue number, if any -->

## Checklist

The pre-push hook and CI enforce these — running them locally first avoids a red PR (see [CONTRIBUTING.md](../CONTRIBUTING.md)):

- [ ] `cargo fmt --check` passes
- [ ] `cargo clippy` is clean
- [ ] `cargo test` passes
- [ ] 100% line coverage holds (`cargo llvm-cov --fail-under-lines 100 --ignore-filename-regex 'src/main\.rs|.*_tests.*\.rs|src/routines/command_builder\.rs|src/cli/restart\.rs|src/cli/start\.rs|src/cli/system\.rs|src/cli/wait_until\.rs|src/machine/write_machine_toml\.rs|src/middlewares/logger\.rs|src/read_runtime_state\.rs|src/routes/http_listener\.rs|src/routes/http_settings_routes\.rs|src/routes/move_routine/cli\.rs|src/routes/move_routine/http\.rs|src/routines/cleanup/log_cap\.rs|src/routines/cleanup/ttl\.rs|src/routines/defaults/write_removed_defaults\.rs|src/routines/next_run_at\.rs|src/routines/service\.rs|src/routines/service_log_tail\.rs|src/routines/service_move\.rs|src/routines/service_update\.rs|src/routines/svc_create\.rs|src/routines/validate_machines\.rs|src/service/macos\.rs|src/service/request_automation_permission\.rs|src/sync/wait_for_crontab_write\.rs|src/utils/atomic\.rs|src/utils/claude_json\.rs|src/run_server\.rs|src/service/linux\.rs'`)
- [ ] Tests live in `*_tests.rs` sibling files (not inline `#[cfg(test)] mod foo { … }` blocks)
- [ ] `CHANGELOG.md` has an entry under `## [Unreleased]` (required for any `src/` or `ui/` change)
- [ ] Docs updated if behavior, CLI flags, or API shapes changed
