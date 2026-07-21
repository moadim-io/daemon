# Changelog

> _Today's TODO, tomorrow's changelog. Ship one, dream up two._

All notable changes to this project are documented here.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

Versions map to the `v*` git tags that drive the crates.io publish workflow.

## [Unreleased]

## [1.6.1] - 2026-07-21

Bump `jiff`, `jiff-static` (0.2.33 → 0.2.34), and `syn` (3.0.0 → 3.0.2) to their latest compatible patch releases (#1314).

test(client): add coverage for `SettingsPage`'s persistent-prompt editor

`SettingsPage.tsx` had zero test coverage (`pnpm --filter client
test:coverage` showed 0% statements/branches/lines) despite carrying real
logic — seeding the draft from the loaded prompt, tracking dirty state, and
gating the Save button on it. Adds `SettingsPage.test.tsx` covering: the
loading state before the prompt query resolves, the textarea seeding from a
loaded prompt with Save disabled until edited, and Save enabling plus the
"unsaved changes" hint once the draft diverges from the loaded value.

No behavior change — test-only.

test: cover `svc_update`'s invalid-env-key reject branch

`svc_update` calls the same `validate_env` used by `svc_create`, but only the `svc_create` side
had a test for the invalid-key rejection branch (`svc_create_rejects_invalid_env_key`). The
`svc_update` call at `validate_env(env)?` in `service_update.rs` was left untested, so
`cargo llvm-cov --fail-under-lines 100` (the repo's own CI and pre-push gate) fell short on that
line.

Adds `svc_update_rejects_invalid_env_key`, mirroring the existing `svc_update_rejects_blank_repository_url`
test shape, asserting the update is rejected with `BadRequest` and the routine's env map is left
untouched. No behavior change; test-only coverage fix.

test(client): add coverage for the routines-page `ViewToggle` (list/calendar/day switcher)

`ViewToggle.tsx` had zero test coverage (`pnpm --filter client test:coverage`
showed 25% statements / 0% branches, lines 16-23 uncovered) even though it
carries real interactive logic — which button renders active and which view
value gets passed back on click. Adds `ViewToggle.test.tsx` covering: all
three buttons render with their labels, only the current view's button gets
the `active` class, and clicking a button calls `onSetView` with that
button's view (including re-clicking the already-active one, which is a
no-op passthrough rather than a toggle-off like `StatsBar`'s facets).

No behavior change — test-only. `ViewToggle.tsx` is now at 100% coverage.

Enable `clippy::cast_possible_wrap`, the mirror image of `clippy::cast_sign_loss`, to reject an `as`-cast from an unsigned integer to a signed one, which silently wraps a value past the target's positive range into a negative one instead of erroring. Fixes the 2 violations this surfaced — `utils::time::format_local`'s Unix-seconds `u64` cast to the `i64` `chrono::Local::timestamp_opt` takes, and a test's `u32` child-process id cast to the `libc::pid_t` (`i32`) `libc::kill` takes — by converting to `<target>::try_from(...)`, clamped to the target's `MAX` on the theoretical overflow case instead of silently wrapping.

Enable `clippy::cast_precision_loss` to catch an `as`-cast from an integer to a floating-point type that can't represent every value of the source exactly (e.g. `u64 as f64`, past 2^52 the cast silently rounds to the nearest representable `f64`), the same "no indication of the invariant" gap `cast_sign_loss` and `cast_possible_wrap` already close for the other integer-cast directions. Fixes the 2 violations this surfaced: `cli::query::humanize_bytes`'s `u64` byte count cast to `f64` for display, which is deliberately an approximation already rounded to one decimal place and is kept as `as f64` behind a scoped, reasoned `#[allow]`; and a test's `delay_ms` shim parameter, narrowed from `u64` to `u32` and converted via `f64::from` so the cast is lossless by construction instead of merely lossless in practice. No behavior change.

Enable `clippy::cast_sign_loss` to reject an `as`-cast from a signed integer to an unsigned one, which silently wraps a negative value into a huge positive one instead of erroring. Fixes the 2 violations this surfaced — a Unix-timestamp `i64` cast to the `u64` seconds this crate stores timestamps as, in `logging::format_json_line` and `ical::feed` — by converting to `u64::try_from(...)` instead.

Enable `clippy::float_cmp` to reject a direct `==`/`!=` comparison between floating-point values. Binary floating-point can't represent most decimal fractions exactly, so two values computed by equivalent-looking paths can differ in the last bit and silently fail an exact-equality check that was meant to test "close enough". The codebase is already clean, so `deny` locks that in. No behavior change.

Enable `clippy::implicit_clone` to reject an indirect `.to_string()`/`.to_owned()`-style clone of a value that's already the target type, in favour of calling `.clone()` directly. The codebase already had zero violations, so this is a lock-in with no behavior change.

Enable `clippy::inefficient_to_string` to reject calling `.to_string()` on a `&&T` (e.g. `&&str`) in favour of calling it on the dereferenced `&T` directly, avoiding an unnecessary extra indirection through `Display`/`ToString`. The codebase already had zero violations, so this is a lock-in with no behavior change.

Enable `clippy::manual_is_variant_and` to reject a `match`/`if let` that manually re-derives what `Option::is_some_and`/`is_none_or` or `Result::is_ok_and`/`is_err_and` already compute (e.g. `match opt { Some(x) => pred(x), None => false }`) instead of calling the combinator directly. The codebase was already clean against this lint; `deny` locks that in.

Enable `clippy::manual_ok_or` to reject a `match`/`if let` that manually converts an `Option` to a `Result` (`Some(x) => Ok(x), None => Err(e)`) instead of `.ok_or(e)`. The codebase was already clean against this lint; `deny` locks that in.

Enable `clippy::missing_const_for_fn` to catch functions whose bodies could run in `const` context but weren't marked `const fn`, closing off callers that need a `const`/`static` initializer or another `const fn` for no reason. Fixes the 7 violations this surfaced (`cli::liveness_exit_code`, `machine::MachineSource::label`, `routes::mcp::MoadimMcp::new`, `routines::cleanup::is_expired`, `routines::flags::FlagScope::suffix`, `routines::model::bool_true`, `routines::service_log_tail::LogWithMeta::empty`) by adding `const`. No behavior change.

Enable `clippy::struct_field_names` to reject a struct field whose name redundantly repeats the struct's own name. Fixes the one violation this surfaced: `Flag::flag_type` (`src/routines/flags.rs`) renamed to `Flag::category`, matching its doc comment ("Free-text category"). The wire format is unchanged — the field already carries `#[serde(rename = "type")]`. No behavior change.

Enable `clippy::verbose_file_reads` to require `fs::read`/`fs::read_to_string` over manual `File::open` + `read_to_end`/`read_to_string`. The two sites that genuinely need the verbose form (reading a log's tail from a seek offset, not the whole file) get a documented `#[allow]`.

test(client): cover failureNotify.ts's localStorage error-handling branch

`pnpm --filter client test:coverage` showed `src/lib/failureNotify.ts`'s
`loadNotifyFailures` `catch` (falling back to notifications-off) untested,
even though it exists specifically to keep a blocked `localStorage`
(private browsing, quota exceeded, disabled storage) from crashing the
failure-notification preference read. Mirrors the same gap already closed
for `theme.ts` (see the `theme-storage-error-coverage` changeset): adds
tests that mock `Storage.prototype.getItem`/`setItem` to throw and assert
the fallback/no-throw behavior each catch is there for.

No behavior change — test-only. `failureNotify.ts` is now at 100% line
coverage.

Fix the web UI rendering a blank page at `GET /` (#1379). Removing the legacy Yew UI (v1.5.0) promoted the React client to the server root, but the client's `BrowserRouter` kept its old `basename="/client"`, so the router matched nothing at `/` and rendered an empty page. The router (and Vite `base`) now resolve from `/`, and old `/client` and `/client/*` links — including query strings like `/client/routines?history=<id>` — permanently redirect to their root-relative equivalents.

chore: move `routine_storage_walk`'s tests out of an inline `#[cfg(test)]` block

`src/routine_storage_walk.rs` held its unit tests in an inline `#[cfg(test)] mod tests { ... }`
block, which the repo's own convention (see CONTRIBUTING.md) and `.githooks/pre-push`'s
test-file-convention check explicitly forbid in favor of `*_tests.rs` sibling files. Nothing in
CI mirrors that specific check (unlike fmt/clippy/coverage/linecheck, which all have a CI job),
so it silently broke `main` for any contributor running the actual local pre-push hook — running
`sh .githooks/pre-push` on HEAD failed at the very first gate with `FAIL:
src/routine_storage_walk.rs: inline test block found (use *_tests.rs instead)`.

Moves the two tests into `src/routine_storage_walk_tests.rs`, matching the `#[path = "..."] mod
..._tests;` pattern already used by every other `_tests.rs` file in the crate. No behavior
change — the tests are unmodified, just relocated so the hook (and any future CI job that mirrors
it) passes again.

Fix stale `ui/src/*.rs` doc-comment references in `client/src/`. `bc9da2e` removed the legacy Yew UI crate (`ui/`) in favor of the React client, but left ~28 doc comments across `client/src/` pointing contributors at now-deleted Rust files as the "ported from" / "see that file for reference behavior" source of truth. Marked each stale reference `(removed)`, and reworded `heatmapMath.ts`'s comment (which told the reader to go check the deleted file for reference behavior) to note the port is now the sole implementation. No behavior change; doc comments only.

Bump the npm dependency group across `client/` (6 updates): `@tanstack/react-query`, `react`, `react-dom`, `react-hook-form`, `typescript-eslint`, and `@changesets/cli`. No behavior change.

fix(client): pin the `@redocly/openapi-core` → `js-yaml` transitive dependency to a patched version

`openapi-typescript` (used by `client`'s `generate:api` script, which `build`/`typecheck`/`lint`/`test`
all run through their `pre*` hooks) pulls in `@redocly/openapi-core@1.34.17`, which resolves
`js-yaml@4.2.0` — a version affected by [GHSA-52cp-r559-cp3m](https://github.com/advisories/GHSA-52cp-r559-cp3m)
(quadratic CPU consumption via YAML merge-key chains), flagged `high` by `pnpm audit`.

Adds a scoped `pnpm.overrides` entry (`"@redocly/openapi-core>js-yaml": ">=4.3.0"`) rather than a
blanket `js-yaml` override, since the tree also carries `js-yaml@3.15.0` via `@changesets/cli`'s
`read-yaml-file` dependency — a global override would force that 3.x consumer onto an incompatible
4.x API. `@redocly/openapi-core` itself already declares `js-yaml: ^4.2.0`, so 4.3.0 satisfies its
own declared range; codegen output (`schema.gen.ts`), `pnpm --filter client typecheck`, `lint`, and
`test` (347 tests) are all unchanged. `pnpm audit` now reports no known vulnerabilities.

Stop generating a `.gitignore` inside every routine directory; the config dir's root `.gitignore` (now also seeding `*.compiled.*` and `run.sh`) covers routine dirs recursively. Existing per-routine `.gitignore` files are left untouched.

Restore the `schedule` field in `routine.toml`. The schedule-to-`schedule.cron` split made the sidecar the source of truth prematurely; `routine.toml` now carries the authoritative `schedule` again (written and read first), while the `schedule.cron` sidecar keeps being written as a mirror of the cron entry (not functional yet). Dirs written during the sidecar-only era still load via a cron-file fallback (comment lines are skipped) until the next repersist heals them, and the JSON Schema requires `schedule` again.

fix: surface a routine's unresolvable `setup`-step interpreter as unhealthy instead of green

`GET /api/v1/routines` responses (`RoutineResponse`) now carry `agent_setup_available`, mirroring
the existing `agent_command_available` field: `true` unless the agent config has a `setup` step
whose first token (the interpreter it shells out to, e.g. `python3` for the built-in `claude`
agent's workspace-trust seeding) does not resolve on the daemon's `PATH`.

Closes the remaining gap from issue #404: `GET /health`'s `dependencies.python3` flag (added in
#902) already told the operator the daemon-wide dependency was missing, but a routine using the
`claude` agent still showed a green "healthy" dot even though its `setup` step — and therefore
the whole run — was guaranteed to fail before the agent ever launched. The UI's `routineHealth()`
now folds `agent_setup_available` into the same "AGENT MISSING" badge `agent_registered` already
uses, since both mean the run aborts without the agent starting.

refactor(routines): split prompt composition out of `command.rs` into `command_prompt.rs`

`src/routines/command.rs` had grown to exactly 500 lines — the ceiling
`linecheck` (the `.githooks/pre-push` line-count gate) enforces — so the very
next change to that file, however small, would have broken CI immediately.

Follows the same pattern this file already uses twice
(`command_path_resolution.rs`, `command_system_prompt.rs`): moves
`compose_prompt`, `substitute`, `placeholder_tokens`,
`validate_placeholders`, `MAX_INLINE_PROMPT_BYTES`, and
`inline_prompt_overflow` — the prompt-composition and
`{placeholder}`-substitution/validation logic — into a new
`src/routines/command_prompt.rs`, re-exported from `command.rs` via
`pub(crate) use command_prompt::*;` so every existing
`crate::routines::command::...` import path is unchanged. `command.rs` drops
to 332 lines.

No behavior change and no new tests needed: existing tests
(`command_tests.rs`, `command_placeholder_tests.rs`) keep passing unmodified
against the re-exported items. `cargo fmt`, `cargo clippy --all-targets`,
`cargo test`, and `cargo llvm-cov --fail-under-lines 100` all pass, and line
coverage stays at 100%.

test(client): cover theme.ts's localStorage error-handling branches

`pnpm --filter client test:coverage` showed `src/lib/theme.ts` at 85.71%
lines: `loadThemeLight`'s `catch` (line 13, falling back to dark) and
`saveThemeLight`'s `catch` (swallowing the write failure) were both
untested, even though they exist specifically to keep a blocked
`localStorage` (private browsing, quota exceeded, disabled storage) from
crashing the theme toggle. Adds two tests that mock `Storage.prototype`'s
`getItem`/`setItem` to throw and assert the fallback/no-throw behavior each
catch is there for.

No behavior change — test-only. `theme.ts` is now at 100% coverage.

Bump `vite` from 6.4.3 to 8.1.5 in `client/`. No behavior change.

Bump `@vitejs/plugin-react` from 5.2.0 to 6.0.3 in `client/`. Requires `vite` ^7 or later, already bumped separately to 8.1.5. No behavior change.

Bump `zod` from 3.25.76 to 4.4.3 in `client/`. No behavior change.

## [1.6.0] - 2026-07-21

Bump `clap` (4.6.2 → 4.6.3) and `tokio` (1.53.0 → 1.53.1) to their latest compatible patch releases. Both are Rust/Cargo dependencies not covered by the existing npm-focused Dependabot PRs.

Bump `cron-union` to `1.0.2` so the routine cron path uses upstream support for `@` aliases and 7-field schedules directly.

test: cover the `spawn_blocking` join-failure branch shared by routine route handlers

`create_routine`, `delete_routine`, `lock_routines`, `trigger_routine`, `unlock_routines`,
`update_routine`, and the scheduled-trigger handler each repeated the same
`tokio::task::spawn_blocking(..).await.map_err(|_| AppError::Internal)??` idiom (#360) to keep
blocking `crontab`/`tmux`/filesystem I/O off the async worker thread. The `map_err` branch — hit
only when the blocking task itself panics — was duplicated seven times and untested at every call
site, since the poison-tolerant stores (`LockRecover`) never actually panic through normal use.
That left `cargo llvm-cov --fail-under-lines 100` (the repo's own CI and pre-push gate) short of
100%.

Extracts the idiom into one `crate::error::run_blocking` helper used by all seven call sites, and
adds direct unit tests for it (including one that deliberately panics the blocking closure) so the
branch is written, and covered, exactly once. No behavior change; purely a dedup + coverage fix.

test: de-genericize the `with_home` test helper in `routine_storage_walk` and add a nested-call test so its `MOADIM_HOME_OVERRIDE` restore branch is exercised, restoring the repo's 100%-line-coverage gate (`cargo llvm-cov --fail-under-lines 100`) to green.

Add an opt-in "Notify on failure" toggle to the Overview page's Recent Runs
section: requests browser notification permission once enabled, then fires a
desktop notification the moment a polled run transitions to `failed` (no
notification for failures already in view when you turn it on).

fix(client): realign `@vitejs/plugin-react`, `react-dom`, and `@types/react-dom` with their peer dependencies

Two prior automated dependency bumps left `client/` with unsatisfiable peer
dependencies: `@vitejs/plugin-react@6.0.3` requires `vite@^8.0.0` (client is
pinned to `vite@^6.0.5`), and `react-dom@19.2.7`/`@types/react-dom@19.2.3`
require `react@^19`/`@types/react@^19` (client is pinned to the `18.3.x`
line). The first broke `vitest` at config-load time
(`ERR_PACKAGE_PATH_NOT_EXPORTED` resolving `vite/internal`); the second, once
unmasked, crashed every test that actually rendered a component
(`react-dom-client.development.js` reading an undefined internal field).
Together they meant `pnpm --filter client test` — part of both CI's
`client-test` job and the local pre-push hook — could not run at all.

Pins `@vitejs/plugin-react` to `^5.2.0` (last major compatible with
`vite@^6`) and `react-dom`/`@types/react-dom` back to the `18.3.x` line
matching `react`/`@types/react`. No application code changed; `pnpm --filter
client test` (329 tests), `typecheck`, and `build` are all green again.

fix(cleanup): `cron_interval_secs` now samples multiple fires to find the schedule's true minimum gap, instead of just the next two — fixes TTL/max-runtime ceilings silently flipping between values depending on wall-clock time for unevenly-spaced multi-fire-per-day schedules (e.g. `"0,30 9 * * *"`).

fix(cleanup): `cron_interval_secs` now applies the same multi-fire minimum-gap sampling to the `cron-union` path, not just the `croner` fallback. Routing schedule math through `cron-union` (#1322) reintroduced the exact "next two fires from now" bug just fixed for the `croner` path (#1323): for an unevenly-spaced multi-fire-per-day schedule like `"0,30 9 * * *"`, the TTL/max-runtime ceiling still flipped between 1800s and 3600s depending on wall-clock time, since `cron-union` compiles almost every schedule (only `@keyword` and 7-field expressions fall back to `croner`). Both branches now go through a shared `min_gap_secs` helper.

test(cli): fix `ensure_readme_returns_early_when_the_path_has_no_parent` to actually exercise its named branch. `Path::new("this-file-should-not-exist").parent()` is `Some("")`, not `None`, so the old test never hit `ensure_readme`'s `parent_or_err` early-return arm — it silently fell through `create_private_dir_all("")` (a no-op) into `std::fs::write`, dropping a stray `this-file-should-not-exist` file into the process's current directory on every `cargo test` run (reproduced: present, untracked, and un-gitignored at the repo root). Switched the test to `Path::new("")`, one of the few paths whose `.parent()` is genuinely `None`, so the branch is actually covered and the run no longer leaves a stray file behind. No production code change.

fix(observability): escape the `machine` label in `GET /api/v1/metrics`'s `moadim_build_info` series for Prometheus text-exposition syntax. `machine` is the only label value in `src/routes/metrics.rs` sourced from free-form user input (`moadim machine set <name>` / `MOADIM_MACHINE`, trimmed but otherwise unrestricted — see `src/machine/mod.rs`); an operator-chosen name containing `"` or `\` was emitted into the label literally, producing unparseable exposition text that would fail the whole scrape, not just that one line. Adds `escape_label_value` (backslash, double-quote, and newline escaping per the exposition format) and applies it to `machine`; `version`/`git_sha` are compile-time constants and don't need it.

Add `*.compiled.*` to the generated routine `.gitignore` so both the legacy `prompt.compiled.md` and current `prompt.compiled.local.md` stay untracked.

refactor(routes): move create_flag HTTP + MCP endpoints into `routes/create_flag`

Follows the `routes/health/` / `routes/shutdown/` / `routes/restart/` /
`routes/get_lock_status/` / `routes/list_agents/` / `routes/cleanup_workbenches/` /
`routes/list_routines/` / `routes/get_routine/` / `routes/delete_routine/` /
`routes/create_routine/` / `routes/list_routine_runs/` / `routes/update_routine/` /
`routes/trigger_routine/` template (see `src/routes/CONTRIBUTING.md`): splits the
`POST /routines/{id}/flags` handler (previously `routines::create_flag` in
`src/routines/handlers.rs`) and the MCP `create_flag` tool into
`src/routes/create_flag/` — `mod.rs` (wiring), `logic.rs` (a `build()` that wraps
`crate::routines::svc_create_flag`, plus the `CreateFlagRequest` request body,
moved out of `routines::handlers`), `http.rs`, and `mcp.rs` (declared as a child
module of `routes::mcp` so it keeps access to `MoadimMcp`'s private state). Both
surfaces now call the same `logic::build()` instead of each hand-calling
`svc_create_flag`.

`list_flags` and `resolve_flag` are left as-is in `routines::handlers` /
`routes::mcp` for now — they're separate MCP tool + REST handler pairs sharing
the same `/routines/{id}/flags` path family, split out in their own follow-up PRs.

No behavior change: same response (the created `Flag`, 201; 400 on an invalid
`type`/`description`/`scope`; 404 when the routine doesn't exist).

refactor(routes): move list_flags HTTP + MCP endpoints into `routes/list_flags`

Follows the `routes/health/` / `routes/shutdown/` / `routes/restart/` /
`routes/get_lock_status/` / `routes/list_agents/` / `routes/cleanup_workbenches/` /
`routes/list_routines/` / `routes/get_routine/` / `routes/delete_routine/` /
`routes/create_routine/` / `routes/list_routine_runs/` / `routes/update_routine/` /
`routes/trigger_routine/` / `routes/create_flag/` template (see
`src/routes/CONTRIBUTING.md`): splits the `GET /routines/{id}/flags` handler
(previously `routines::list_flags` in `src/routines/handlers.rs`) and the MCP
`list_flags` tool into `src/routes/list_flags/` — `mod.rs` (wiring), `logic.rs`
(a `build()` that wraps `crate::routines::svc_list_flags`), `http.rs`, and
`mcp.rs` (declared as a child module of `routes::mcp` so it keeps access to
`MoadimMcp`'s private state). Both surfaces now call the same `logic::build()`
instead of each hand-calling `svc_list_flags`.

`resolve_flag` is left as-is in `routines::handlers` / `routes::mcp` for
now — it's a separate MCP tool + REST handler pair sharing the same
`/routines/{id}/flags/{filename}` path family, split out in its own follow-up
PR.

No behavior change: same response (`Vec<Flag>`, 200; 404 when the routine
doesn't exist).

refactor(routes): move lock_routines HTTP + MCP endpoints into `routes/lock_routines`

Follows the `routes/health/` / `routes/shutdown/` / `routes/restart/` /
`routes/get_lock_status/` / `routes/list_agents/` / `routes/cleanup_workbenches/` /
`routes/list_routines/` / `routes/get_routine/` / `routes/delete_routine/` /
`routes/create_routine/` / `routes/list_routine_runs/` / `routes/update_routine/` /
`routes/trigger_routine/` / `routes/create_flag/` / `routes/list_flags/` /
`routes/resolve_flag/` template (see `src/routes/CONTRIBUTING.md`): splits the
`POST /routines/lock` handler (previously `routines::lock` in
`src/routines/handlers.rs`, named `lock`) and the MCP `lock_routines` tool
into `src/routes/lock_routines/` — `mod.rs` (wiring), `logic.rs` (a `build()`
that validates the scope, creates the lock sentinel, and syncs the crontab),
`http.rs` (renamed handler `lock_routines`, still offloading to
`spawn_blocking` since the crontab sync shells out to `crontab`(1)), and
`mcp.rs` (declared as a child module of `routes::mcp` so it keeps access to
`MoadimMcp`'s private state). Both surfaces now call the same
`logic::build()` instead of each hand-validating the scope and syncing the
crontab separately.

`unlock`/`unlock_routines` are unchanged and still live in
`src/routines/handlers.rs` / `routes/mcp.rs` — a future PR will split those
out the same way.

No behavior change: same response (the current lock status), 400 on an
unknown scope, 500 on IO failure.

refactor(routes): move resolve_flag HTTP + MCP endpoints into `routes/resolve_flag`

Follows the `routes/health/` / `routes/shutdown/` / `routes/restart/` /
`routes/get_lock_status/` / `routes/list_agents/` / `routes/cleanup_workbenches/` /
`routes/list_routines/` / `routes/get_routine/` / `routes/delete_routine/` /
`routes/create_routine/` / `routes/list_routine_runs/` / `routes/update_routine/` /
`routes/trigger_routine/` / `routes/create_flag/` / `routes/list_flags/` template
(see `src/routes/CONTRIBUTING.md`): splits the `DELETE
/routines/{id}/flags/{filename}` handler (previously `routines::resolve_flag` in
`src/routines/handlers.rs`) and the MCP `resolve_flag` tool into
`src/routes/resolve_flag/` — `mod.rs` (wiring), `logic.rs` (a `build()` that wraps
`crate::routines::svc_resolve_flag`), `http.rs`, and `mcp.rs` (declared as a child
module of `routes::mcp` so it keeps access to `MoadimMcp`'s private state). Both
surfaces now call the same `logic::build()` instead of each hand-calling
`svc_resolve_flag`.

No behavior change: same response (204 on success, error on a missing routine or
flag).

Move the routine-origin disclosure into the compiled prompt body so it shows up in prompt previews and `prompt.compiled.local.md` instead of being injected only at launch time.

refactor(routes): move trigger_routine HTTP + MCP endpoints into `routes/trigger_routine`

Follows the `routes/health/` / `routes/shutdown/` / `routes/restart/` /
`routes/get_lock_status/` / `routes/list_agents/` / `routes/cleanup_workbenches/` /
`routes/list_routines/` / `routes/get_routine/` / `routes/delete_routine/` /
`routes/create_routine/` / `routes/list_routine_runs/` / `routes/update_routine/`
template (see `src/routes/CONTRIBUTING.md`): splits the `POST
/routines/{id}/trigger` handler (previously `routines::trigger` in
`src/routines/handlers.rs`) and the MCP `trigger_routine` tool into
`src/routes/trigger_routine/` — `mod.rs` (wiring), `logic.rs` (a `build()`
that wraps `crate::routines::svc_trigger`), `http.rs` (still offloading to
`spawn_blocking` since `svc_trigger` shells out to `tmux`(1) and does blocking
fs I/O), and `mcp.rs` (declared as a child module of `routes::mcp` so it keeps
access to `MoadimMcp`'s private state). Both surfaces now call the same
`logic::build()` instead of each hand-calling `svc_trigger`.

No behavior change: same response (the triggered routine record, 423 when
disabled or in power-saving mode, 404 when missing).

refactor(routes): move unlock_routines HTTP + MCP endpoints into `routes/unlock_routines`

Follows the `routes/health/` / `routes/shutdown/` / `routes/restart/` /
`routes/get_lock_status/` / `routes/list_agents/` / `routes/cleanup_workbenches/` /
`routes/list_routines/` / `routes/get_routine/` / `routes/delete_routine/` /
`routes/create_routine/` / `routes/list_routine_runs/` / `routes/update_routine/` /
`routes/trigger_routine/` / `routes/create_flag/` / `routes/list_flags/` /
`routes/resolve_flag/` / `routes/lock_routines/` template (see
`src/routes/CONTRIBUTING.md`): splits the `DELETE /routines/lock` handler
(previously `routines::unlock` in `src/routines/handlers.rs`) and the MCP
`unlock_routines` tool into `src/routes/unlock_routines/` — `mod.rs`
(wiring), `logic.rs` (a `build()` that validates the scope — now including
`"all"` in the single shared parser instead of special-casing it at each
call site — removes the matching lock sentinel(s), and syncs the crontab),
`http.rs` (renamed handler `unlock_routines`, still offloading to
`spawn_blocking` since the crontab sync shells out to `crontab`(1)), and
`mcp.rs` (declared as a child module of `routes::mcp` so it keeps access to
`MoadimMcp`'s private state). Both surfaces now call the same
`logic::build()` instead of each hand-validating the scope and syncing the
crontab separately.

`snooze_routine` and `set_power_saving` have no REST counterpart, so they
stay as-is in `routes/mcp.rs`.

No behavior change: same response (the current lock status), 400 on an
unknown scope, 500 on IO failure.

feat(routines): support nested routine folders — a routine's `title` may contain `/`-separated
path segments (e.g. `ops/nightly triage`), organizing routines into folders and subfolders in the
UI. `slugify` now preserves `/` as a segment separator instead of collapsing it, so nested titles
produce nested, filesystem- and tmux-safe slugs.

feat(routines): per-routine environment variables via `[env]` + `routine.local.toml` (#408)

test(routine_storage): cover `read_routine_cron`'s empty-file branch — a `schedule.cron` sidecar with no non-empty line (e.g. truncated by a crash mid-write) now has a regression test asserting the whole routine load short-circuits to `None` instead of silently loading with a blank schedule, closing a gap in the pre-push 100%-line-coverage gate.

test(routines): cover `rotate_run_history_if_oversized`'s rotated-write-failure branch — when the `.1` sibling can't be written (e.g. it turns out to be a directory), the oversized source `runs.log` must survive rather than being removed, since removal is gated on the write actually succeeding. This branch previously had no test, unlike its sibling I/O-failure branches elsewhere in `cleanup::log_cap`.

Use `cron-union` for routine schedule math so the routine cron path already routes through the union helper that future multi-cron support can build on.

## [1.5.0] - 2026-07-20

Add a built-in `pi` agent config and seed `pi.toml` alongside the existing `claude`, `codex`, and `hermes` defaults.

Bump `tokio` from 1.52.4 to 1.53.0 (permitted by the existing `"1"` version requirement; the lockfile was simply stale). No API or behavior changes.

test: cover the "path has no parent directory" error branches that `cargo llvm-cov`'s 100% line floor was failing on. Five call sites (`write_pid_file`, `ensure_readme`, `spawn_detached_with` in `cli/system.rs`, `write_machine_toml` in `machine/mod.rs`, `write_removed_defaults` in `routines/defaults/mod.rs`, and `append_persisted_run` in `routines/run_history.rs`) each duplicated their own `path.parent().ok_or_else(...)`/`let Some(parent) = ... else { ... }` guard for a case none of this crate's real config/log/history paths ever hit. Extracted the shared guard into `utils::fs_perms::parent_or_err`, tested directly, so the duplicate unreachable branches at each call site disappear instead of each needing its own contrived test. Also adds a test for the sibling "unparsable bind address" branch in `http_request_core`. Cuts the coverage gate's missed-line count from 32 to 6; the remainder is an effectively unreachable `serde_json::to_string` failure branch in `run_history.rs` and two unrelated single-line gaps, left for a follow-up.

chore(lint): enable `clippy::expect_used` in the root crate, forbidding `.expect()` in production code. `.expect()` panics exactly like `.unwrap()` (already forbidden via `unwrap_used`) — it only adds a custom message to the same daemon-killing failure mode, so `unwrap_used` alone left it an unguarded back door. Fixed the 25 violations this surfaced: most now propagate a proper `Result` (`?`, `ok_or_else`, or a `let else` early return); a handful of genuinely provable "can't happen" invariants (an id checked to exist earlier in the same function with the lock held continuously since; `Stdio::piped()` set a few lines above the matching `.take()`) are kept as `.expect()` behind a scoped, reasoned `#[allow]`. `build.rs` and its `src/build/` helper modules carry their own crate-level exemption — a build script's `.expect()` panic just aborts `cargo build` with a message, the same intended failure mode as test code.

chore(lint): enable `clippy::ignore_without_reason` in the root crate, forbidding a bare `#[ignore]` on a test with no explanation of why it's skipped. Same rationale as the existing `allow_attributes_without_reason` lint, applied to test-skipping instead of lint-suppression: a silently ignored test rots invisibly unless the reason is written down next to it (e.g. `#[ignore = "requires a live tmux session"]`). No `#[ignore]` exists in the codebase today, so this adds no diff beyond the lint config — it just locks in that any future one must justify itself.

Enable `clippy::manual_assert` workspace-wide, continuing this crate's incremental clippy-lint
enablement (see `enable-clippy-expect-used`, the `redundant_type_annotations`/
`unseparated_literal_suffix` lints, etc.). Rejects `if !cond { panic!("msg") }` in favour of
`assert!(cond, "msg")`, which states the invariant directly instead of making the reader invert
the condition. The codebase already had zero violations, so this is a lint-only change with no
behavior change.

Enable `clippy::panic` workspace-wide to forbid `panic!()` in production code, matching the existing `unwrap_used`/`expect_used` hardening against unhandled panics in the long-running daemon process. Test code stays exempt via `allow-panic-in-tests` in `clippy.toml`.

chore(lint): enable `clippy::redundant_type_annotations` workspace-wide

Rejects a `let` binding whose explicit type annotation exactly matches what the compiler would
already infer — the annotation adds nothing beyond what the initializer already states, so it's
noise a reader has to cross-check against the inferred type instead of trusting. Enabled in both
the root `Cargo.toml` and `ui/Cargo.toml` (the `ui` crate has its own `[lints.clippy]` table and
doesn't inherit root's deny-list), mirroring the existing lint-parity pattern.

The workspace (both `src/` and `ui/src`) was already clean, so no fixes were needed — `deny`
just locks that in.

chore(lint): enable `clippy::tests_outside_test_module` workspace-wide

Rejects a `#[test]` function declared outside a `#[cfg(test)]` module. This formalizes, at the
AST level, this repo's existing `*_tests.rs` convention: `.githooks/pre-push` step 1 already
greps for tests living outside a `#[cfg(test)] mod foo_tests;` sibling, but that check is
text-pattern-based and only runs locally, pre-push. Enabling this lint means `cargo clippy`
(CI's `lint.yml` job) enforces the same invariant from the compiler's own AST, so it can't be
skipped by a contributor who bypasses git hooks.

Enabled in both the root `Cargo.toml` and `ui/Cargo.toml` (the `ui` crate has its own
`[lints.clippy]` table and doesn't inherit root's deny-list), mirroring the parity pattern
already used for `unreachable`, `redundant_type_annotations`, and others. The workspace
(`src/` and `ui/src`) was already clean, so `deny` locks that in with no behavior change.

chore(lint): enable `clippy::undocumented_unsafe_blocks` workspace-wide, requiring a reasoned `// SAFETY:` comment for every `unsafe` block. This locks in the existing convention as a compiler-checked rule instead of an unenforced habit.

chore(lint): enable `clippy::unreachable` workspace-wide

Rejects the `unreachable!()` macro: a daemon process (or the running Yew UI) that hits one
panics instead of returning a structured error, and a match arm that looks provably impossible
today can become reachable after an innocuous refactor elsewhere — the panic then only surfaces
at runtime, in production. Enabled in both the root `Cargo.toml` and `ui/Cargo.toml` (the `ui`
crate has its own `[lints.clippy]` table and doesn't inherit root's deny-list), mirroring the
pattern of prior lint-parity chores.

Fixed the one violation this surfaced: `src/build/ui.rs`'s `base64_encode` matched on a 3-byte
chunk's length with a `_ => unreachable!()` catch-all for lengths other than 1/2/3 (impossible
from `bytes.chunks(3)`). Rewritten without a `match` — the guaranteed-present first byte is
indexed directly and the optional second/third bytes are read via `.get()` — so there's no
catch-all arm left to guard, and the output is unchanged. The rest of the workspace (including
`ui/src`) was already clean, so this locks that in with `deny`.

chore(lint): enable `clippy::unseparated_literal_suffix` workspace-wide

Denies a numeric literal whose type suffix isn't underscore-separated from its digits (e.g.
`500u64` instead of `500_u64`), matching the existing `unreadable_literal` convention for
large integer literals. Enabled in both the root `Cargo.toml` and `ui/Cargo.toml` (the `ui`
crate has its own `[lints.clippy]` table and doesn't inherit root's deny-list). Fixed the 48
violations this surfaced (27 root, 21 `ui`) via `cargo clippy --fix` — all mechanical
suffix-underscore insertions, no behavior change.

fix(client): restore a working `client/` build (broken on `main`, `client (vitest)` CI red for 3+ pushes)

Two independent, pre-existing dependency breaks, surfaced while making the React client the sole
UI (see the "remove the legacy Yew UI" changeset):

- `@vitejs/plugin-react@6.0.3` (bumped in #1191) peer-requires `vite@^8.0.0`, but this repo pins
  `vite@^6.0.5`. Vite's own package no longer exposes the `./internal` subpath 6.0.3 imports,
  so `vite build`/`vitest` failed to even load `vite.config.ts`. Downgraded to `@vitejs/plugin-react@^5.2.0`,
  the latest release still compatible with `vite ^6`.
- `react-dom` was bumped to `^19.2.7` in #1187 without bumping `react` itself, which stayed at
  `^18.3.1` — a cross-major mismatch that crashes on mount (`Cannot read properties of undefined
  (reading 'S')`) the moment the vite/plugin-react fix above let tests actually run. Bumped `react`
  and `@types/react` to `^19.2.7`/`^19.2.3` to match. React 19's stricter `RefObject<T>` typing
  (no longer implicitly nullable) surfaced one real type error: `FilterBarProps.searchRef` is now
  `RefObject<HTMLInputElement | null>`, matching what `useRef<HTMLInputElement>(null)` actually
  returns.

No intentional behavior change; `pnpm --filter client build/typecheck/lint/test` are all green
again.

fix(client): resolve `client-lint`'s 8 `react-hooks/purity` and `react-hooks/set-state-in-effect` errors

The `eslint-plugin-react-hooks` bump to 7.1.1 (#1185) enabled stricter React
Compiler rules that flag pre-existing code: four `Date.now()` calls during
render (`react-hooks/purity`) and four `setState` calls synchronously inside
a `useEffect` (`react-hooks/set-state-in-effect`). This left `pnpm --filter
client lint` — part of both CI's `client-lint` job and the local pre-push
hook — failing on `main` for any contributor who runs it, independent of
what their own change touches.

- Added a shared `useNow()` hook (`client/src/lib/useNow.ts`) that reads the
  clock inside a timer effect instead of during render, and reused it in
  `RefreshControl`, `RoutineFlags`, and `RoutineHistory` (which previously
  each read `Date.now()` directly in their render body).
- Moved four `setState` calls (`RoutinesPage`'s deep-link page and stale-selection
  prune, `SettingsPage`'s draft-seeding, `CommandPalette`'s reset-on-open) out
  of `useEffect` and into a lazy `useState` initializer or a guarded
  render-time update, per React's own "Adjusting state when a prop changes"
  guidance — no behavior change intended.

No dependency versions changed here; unrelated to #1251, which fixes a
separate `@vitejs/plugin-react`/`vite` peer-dependency break that currently
also blocks `pnpm --filter client test` from even starting.

fix(coverage): close the 100%-line-coverage gap left by 3 untestable error branches

Pre-existing on `main` (`cargo llvm-cov (100% line floor)` CI red for 3+ pushes), surfaced while
fixing `client (vitest)` (see the other changeset in this PR) — `std::env::current_exe()` failing
is otherwise unreachable in a test (the syscall only errors if the running binary's own file was
deleted mid-execution, or under unusual sandboxing), and re-serializing a `serde_json::Value` just
parsed from valid JSON text is unreachable too (the only failure mode is a non-finite float, which
JSON's grammar cannot express).

Added two test-only env-var seams, mirroring the existing `MOADIM_CRONTAB_BIN`/
`MOADIM_LAUNCHCTL_BIN` pattern for external-binary resolution:

- `utils::process::current_exe()` wraps `std::env::current_exe`, honoring
  `MOADIM_CURRENT_EXE_FAIL_FOR_TEST` in test builds. Used by `service::common::moadim_exe` and
  `cli::system::spawn_detached_with` (both call sites needed their own test, since the generic
  `spawn_detached_with`'s error-mapping closure is monomorphized separately per caller).
- `utils::claude_json::serialize_document()` wraps `serde_json::to_vec`, honoring
  `MOADIM_CLAUDE_JSON_SERIALIZE_FAIL_FOR_TEST`.

No behavior change outside `#[cfg(test)]`.

Crontab block replacement now matches its delimiters as whole lines instead of raw substrings, guarding against a marker prefix-matching a more specific one elsewhere in the crontab and silently overwriting it. (#324)

fix(client): resolve `RoutineForm`'s `react-hooks/incompatible-library` warning

`useForm().watch()` called with no arguments returns a subscription function
whose identity isn't stable across renders, so React Compiler's
`eslint-plugin-react-hooks` bails out of memoizing the whole component.
`RoutineForm` was the only place in the client tree using this pattern.

Replaced the bulk `watch()` call with scoped `useWatch({ control, name })`
calls for the five fields actually read (`title`, `schedule`, `agent`,
`prompt`, `machines`) — react-hook-form's recommended reactive-subscription
hook for this exact case. No behavior change; `pnpm --filter client lint`
now reports 0 warnings.

fix(routines): stop `runs.log` rotation from silently discarding prior run history (#1277)

A routine's durable `runs.log` is rotated to a sibling `runs.log.1` once it
crosses 1 MiB (`RUN_HISTORY_MAX_BYTES`), but the rotation used a bare
`fs::rename`, which **overwrites** any existing `.1` file. Combined with
`read_persisted_runs` only ever reading the current `runs.log`, every
rotation past the first permanently discarded that routine's history —
despite `runs.log` being documented as durable history that survives
workbench TTL reaping.

- `rotate_run_history_if_oversized` now merges the rotating-out content onto
  the end of any existing `.1` file instead of overwriting it.
- `read_persisted_runs` now reads both `runs.log` and `runs.log.1` and
  merges the results, so `GET /routines/{id}/runs` / `GET /routines/runs`
  and the UI views built on them (history, Overview, Reliability rankings)
  no longer lose history across a rotation.
- Added a regression test exercising rotation followed by a read, asserting
  pre-rotation entries are still visible afterward.

Avoid a redundant `String` clone in `ensure_config_gitignore()`: `existing` is only borrowed (via `lines()`) before the clone site and is never read again afterward, so the buffer can be moved into `content` instead of cloned. No behavior change; this runs on every daemon start/restart.

refactor(routes): move cleanup_workbenches HTTP + MCP endpoints into `routes/cleanup_workbenches`

Follows the `routes/health/` / `routes/shutdown/` / `routes/restart/` /
`routes/get_lock_status/` / `routes/list_agents/` template (see
`src/routes/CONTRIBUTING.md`): splits the `POST /routines/cleanup` handler
(previously `cleanup` in `src/routines/handlers.rs`) and the MCP
`cleanup_workbenches` tool into `src/routes/cleanup_workbenches/` — `mod.rs`
(wiring), `logic.rs` (a `build()` that wraps `crate::routines::svc_cleanup()`
and re-exports `CleanupResponse`), `http.rs`, and `mcp.rs` (declared as a
child module of `routes::mcp` so it keeps access to `MoadimMcp`'s private
state). Both surfaces now call the same `logic::build()` instead of each
calling `svc_cleanup()` separately.

No behavior change: same response shape (`removed`, `freed_bytes`), same
`spawn_blocking` wrapping around the blocking fs/tmux sweep on the HTTP side.

refactor(routes): move create_routine HTTP + MCP endpoints into `routes/create_routine`

Follows the `routes/health/` / `routes/shutdown/` / `routes/restart/` /
`routes/get_lock_status/` / `routes/list_agents/` / `routes/cleanup_workbenches/` /
`routes/list_routines/` / `routes/get_routine/` / `routes/delete_routine/` template
(see `src/routes/CONTRIBUTING.md`): splits the `POST /routines` handler
(previously `routines::create` in `src/routines/handlers.rs`) and the MCP
`create_routine` tool into `src/routes/create_routine/` — `mod.rs` (wiring),
`logic.rs` (a `build()` that wraps `crate::routines::svc_create`), `http.rs`
(keeps the `spawn_blocking` offload since `svc_create` syncs the crontab), and
`mcp.rs` (declared as a child module of `routes::mcp` so it keeps access to
`MoadimMcp`'s private state). Both surfaces now call the same `logic::build()`
instead of each hand-calling `svc_create`.

No behavior change: same response (the created routine record, 400 on an
invalid cron expression).

refactor(routes): move delete_routine HTTP + MCP endpoints into `routes/delete_routine`

Follows the `routes/health/` / `routes/shutdown/` / `routes/restart/` /
`routes/get_lock_status/` / `routes/list_agents/` / `routes/cleanup_workbenches/` /
`routes/list_routines/` / `routes/get_routine/` template (see
`src/routes/CONTRIBUTING.md`): splits the `DELETE /routines/{id}` handler
(previously `routines::delete` in `src/routines/handlers.rs`) and the MCP
`delete_routine` tool into `src/routes/delete_routine/` — `mod.rs` (wiring),
`logic.rs` (a `build()` that wraps `crate::routines::svc_delete`), `http.rs`
(keeps the `spawn_blocking` offload since `svc_delete` syncs the crontab), and
`mcp.rs` (declared as a child module of `routes::mcp` so it keeps access to
`MoadimMcp`'s private state). Both surfaces now call the same `logic::build()`
instead of each hand-calling `svc_delete`.

No behavior change: same response (the deleted routine record, 404 when missing).

refactor(routes): move get_lock_status HTTP + MCP endpoints into `routes/get_lock_status`

Follows the `routes/health/` / `routes/shutdown/` / `routes/restart/` template
(see `src/routes/CONTRIBUTING.md`): splits the `GET /routines/lock` handler
(previously in `src/routines/handlers.rs`) and the MCP `get_lock_status` tool
into `src/routes/get_lock_status/` — `mod.rs` (wiring), `logic.rs` (a
`build()` that wraps `crate::global_lock::lock_status()`), `http.rs`, and
`mcp.rs` (declared as a child module of `routes::mcp` so it keeps access to
`MoadimMcp`'s private state). Both surfaces now call the same `logic::build()`
instead of each calling `crate::global_lock::lock_status()` separately.

No behavior change: same response fields (`shared`, `local`, `locked`).

refactor(routes): move get_routine HTTP + MCP endpoints into `routes/get_routine`

Follows the `routes/health/` / `routes/shutdown/` / `routes/restart/` /
`routes/get_lock_status/` / `routes/list_agents/` / `routes/cleanup_workbenches/` /
`routes/list_routines/` template (see `src/routes/CONTRIBUTING.md`): splits the
`GET /routines/{id}` handler (previously `routines::get` in
`src/routines/handlers.rs`) and the MCP `get_routine` tool into
`src/routes/get_routine/` — `mod.rs` (wiring), `logic.rs` (a `build()` that
wraps `crate::routines::svc_get`), `http.rs`, and `mcp.rs` (declared as a child
module of `routes::mcp` so it keeps access to `MoadimMcp`'s private state).
Both surfaces now call the same `logic::build()` instead of each hand-calling
`svc_get`.

No behavior change: same response (a single routine by UUID, 404 when missing).

refactor(routes): move list_agents HTTP + MCP endpoints into `routes/list_agents`

Follows the `routes/health/` / `routes/shutdown/` / `routes/restart/` /
`routes/get_lock_status/` template (see `src/routes/CONTRIBUTING.md`): splits
the `GET /agents` handler (previously in `src/routines/handlers.rs`) and the
MCP `list_agents` tool into `src/routes/list_agents/` — `mod.rs` (wiring),
`logic.rs` (a `build()` that wraps `crate::routines::available_agents()`),
`http.rs`, and `mcp.rs` (declared as a child module of `routes::mcp` so it
keeps access to `MoadimMcp`'s private state). Both surfaces now call the same
`logic::build()` instead of each calling `available_agents()` separately.

No behavior change: same response (array of available agent registry keys).

refactor(routes): move list_routine_runs HTTP + MCP endpoints into `routes/list_routine_runs`

Follows the `routes/health/` / `routes/shutdown/` / `routes/restart/` /
`routes/get_lock_status/` / `routes/list_agents/` / `routes/cleanup_workbenches/` /
`routes/list_routines/` / `routes/get_routine/` / `routes/delete_routine/` /
`routes/create_routine/` template (see `src/routes/CONTRIBUTING.md`): splits the
`GET /routines/{id}/runs` handler (previously `routines::get_runs` in
`src/routines/handlers.rs`) and the MCP `list_routine_runs` tool into
`src/routes/list_routine_runs/` — `mod.rs` (wiring), `logic.rs` (a `build()`
that wraps `crate::routines::svc_list_runs`), `http.rs`, and `mcp.rs`
(declared as a child module of `routes::mcp` so it keeps access to
`MoadimMcp`'s private state). Both surfaces now call the same `logic::build()`
instead of each hand-calling `svc_list_runs`.

No behavior change: same response (a routine's runs, newest first, 404 when
the routine is missing).

refactor(routes): move list_routines HTTP + MCP endpoints into `routes/list_routines`

Follows the `routes/health/` / `routes/shutdown/` / `routes/restart/` /
`routes/get_lock_status/` / `routes/list_agents/` / `routes/cleanup_workbenches/`
template (see `src/routes/CONTRIBUTING.md`): splits the `GET /routines` handler
(previously `routines::list` in `src/routines/handlers.rs`) and the MCP
`list_routines` tool into `src/routes/list_routines/` — `mod.rs` (wiring),
`logic.rs` (a `build()` that wraps `crate::routines::svc_list`), `http.rs`,
and `mcp.rs` (declared as a child module of `routes::mcp` so it keeps access
to `MoadimMcp`'s private state). Both surfaces now call the same
`logic::build()` instead of each hand-assembling the same call to `svc_list`.

No behavior change: same response (routine list, `local_only`/`include_prompts`
still respected on both surfaces).

refactor(routes): move update_routine HTTP + MCP endpoints into `routes/update_routine`

Follows the `routes/health/` / `routes/shutdown/` / `routes/restart/` /
`routes/get_lock_status/` / `routes/list_agents/` / `routes/cleanup_workbenches/` /
`routes/list_routines/` / `routes/get_routine/` / `routes/delete_routine/` /
`routes/create_routine/` / `routes/list_routine_runs/` template (see
`src/routes/CONTRIBUTING.md`): splits the `PATCH /routines/{id}` handler
(previously `routines::update` in `src/routines/handlers.rs`, with
`routines::replace` as its `PUT` alias) and the MCP `update_routine` tool into
`src/routes/update_routine/` — `mod.rs` (wiring), `logic.rs` (a `build()` that
wraps `crate::routines::svc_update`), `http.rs` (keeps both the `PATCH` handler
and the `PUT` alias, still offloading to `spawn_blocking` since `svc_update`
syncs the crontab), and `mcp.rs` (declared as a child module of `routes::mcp`
so it keeps access to `MoadimMcp`'s private state). Both surfaces now call the
same `logic::build()` instead of each hand-calling `svc_update`.

No behavior change: same response (the updated routine record, 400 on invalid
fields, 404 when missing).

feat(observability): add `GET /api/v1/metrics`, a Prometheus text-exposition endpoint (#414)

Exposes `moadim_uptime_seconds`, `moadim_build_info`, `moadim_active_sessions`,
`moadim_workbench_bytes`, `moadim_runs_total{status=...}`,
`moadim_run_duration_seconds` (histogram), `moadim_cleanup_removed_total`, and
`moadim_cleanup_freed_bytes_total`. Run counts/durations and active sessions are
derived at scrape time from the same durable run history (`runs.log` + live
workbenches) and live tmux session count the REST "recent runs" view and the
concurrency cap already read, rather than a second, parallel counter that could
drift from it. Cleanup-sweep totals are tracked as process-lifetime atomics,
incremented at the one function both the periodic sweep and the on-demand
`POST /routines/cleanup` route already funnel through, so they reflect real
sweeps and not just on-demand snapshots. `GET /health` is unchanged — it stays
the cheap liveness probe, `/metrics` is the richer scrape surface.

feat(ui): remove the legacy Yew UI (`ui/`), make the React client (`client/`) the sole embedded UI

The daemon has shipped two parallel browser UIs since the React client's rollout began: the
original Yew/WASM SPA at `/` and the React client at `/client`. The Yew crate is now removed and
the React client takes over `/` as the one and only UI — no more dual-maintenance, dual-build, or
dual-CSP-allowance burden.

- Deleted the `ui/` Cargo workspace member (Yew/WASM crate) entirely, along with its Trunk build
  step (`src/build/ui.rs`), workspace membership (`Cargo.toml`), and CI jobs (`clippy-ui-wasm`,
  `prebuilt-ui.yml`).
- `src/build/client.rs` (the former `/client` builder) is now the only UI builder — it writes
  `$OUT_DIR/index.html` / `prebuilt.html` directly, replacing the old Yew-inlining build step.
- `GET /` now serves the React client; the `/client` route and its nested router are gone.
- `prebuilt-client.html` renamed to `prebuilt.html` (the old Yew `prebuilt.html` is deleted); its
  freshness-check workflow renamed `prebuilt-ui.yml` → `prebuilt.yml`.
- Dropped `'wasm-unsafe-eval'` from the CSP's `script-src` — no WASM SPA is served anymore, so the
  narrower policy is strictly tighter than before.
- Updated `Architecture.md`, `CONTRIBUTING.md`, `.githooks/pre-push`, and the remaining CI
  workflows (`lint.yml`, `test.yml`, `publish.yml`, `changelog.yml`) to drop every `ui/` reference.

No REST/MCP API changes. The `/ui` back-compat redirect to `/` is unaffected.

fix(routines): fold `run_history`'s serialize failure into its existing best-effort append chain, closing the coverage gate's last `run_history.rs` gap. `append_persisted_run` logged and returned early on a `serde_json::to_string` failure via its own dedicated `match`, separate from the `Result` chain already covering directory-creation/open/write failures for the same best-effort append — and since `PersistedRun`'s fields can never actually fail to serialize, that separate branch was untestable, leaving 3 lines permanently below `cargo llvm-cov`'s 100% line floor. Folding it into the same chain (one log call, one failure path, matching the function's own doc comment) removes the untestable branch entirely instead of contriving a test for it. This was the last of three follow-ups named by #1268; the remaining two (`cli/system.rs`, `service/common.rs`, `utils/claude_json.rs`, one line each) are unrelated and left for their own PRs.

Add a Reliability page to the React client, ranking routines by success rate, failure streak, and flakiness, with per-routine p50/p95 run duration and a slower-trend regression flag. Frontend-only — reads the existing `GET /routines/runs` payload.

## [1.4.1] - 2026-07-18

chore(lint): enable clippy::exit workspace-wide, forbidding `std::process::exit`/`std::process::abort` outside `fn main`. Prevents a `Drop`-skipping process termination (leaked lock guards, file handles, in-flight routine cleanup) from a long-running daemon code path other than the CLI's top-level dispatch. Codebase was already clean; no fixes needed.

## [1.4.0] - 2026-07-18

fix(security): refuse to start on a non-loopback `MOADIM_BIND_ADDR` unless `MOADIM_ALLOW_REMOTE=1` is explicitly set (#253)

chore(deps): bump cron-parser from 4.9.0 to 5.6.2

chore(deps-dev): bump eslint-plugin-react-hooks from 5.2.0 to 7.1.1

chore(deps): bump react-dom and @types/react-dom

chore(deps): bump react-router-dom from 6.30.4 to 7.18.1

chore(deps-dev): bump @vitejs/plugin-react from 4.7.0 to 6.0.3

fix(routines): serialize the default-routine tombstone file's writes to close a lost-update race

`record_removed_default` and `clear_removed_default` each read the whole `removed_defaults.local.toml`
tombstone file, mutate the slug set, and write it back in full, with no synchronization between the
two. `DELETE /routines/{id}` and `POST /routines` (which call them from `svc_delete`/`svc_create`
respectively) can be handled concurrently on the multi-thread Tokio runtime, so two overlapping
read-modify-write round trips could interleave and the later write would silently drop whichever
change the other request had just persisted — e.g. deleting two different built-in default routines
back to back could lose one tombstone, resurrecting a routine the user explicitly removed on the
next daemon startup (the same hazard class as the crontab read-modify-write race fixed in issue
#365, and the `machine.local.toml` race fixed in #1240). Both functions now serialize through a
single `Mutex`, mirroring the existing `crontab_sync_lock`/`machine_toml_lock` pattern, so concurrent
tombstone writes can no longer clobber each other.

chore(lint): enable `clippy::format_collect` in the `ui` crate

Mirrors the root crate's `format_collect = "deny"` (root `Cargo.toml`) — the `ui` crate has its
own `[lints.clippy]` table and doesn't inherit root's extended deny-list, so this never applied to
`ui/src` despite CI's `clippy` job running `--workspace`. The `ui` crate was already clean, so this
surfaced 0 violations. No behavior change.

chore(lint): enable `clippy::large_stack_arrays` in the `ui` crate

Mirrors the root crate's `large_stack_arrays = "deny"` (root `Cargo.toml`) — the `ui` crate has
its own `[lints.clippy]` table and doesn't inherit root's extended deny-list, so this never
applied to `ui/src` despite CI's `clippy` job running `--workspace`. The `ui` crate was already
clean, so this surfaced 0 violations. No behavior change.

chore(lint): enable `clippy::literal_string_with_formatting_args` in the `ui` crate

Mirrors the root crate's `literal_string_with_formatting_args = "deny"` (root `Cargo.toml`) —
the `ui` crate has its own `[lints.clippy]` table and doesn't inherit root's extended deny-list,
so this never applied to `ui/src` despite CI's `clippy` job running `--workspace`. The `ui` crate
was already clean, so this surfaced 0 violations. No behavior change.

chore(lint): enable `clippy::similar_names` in the `ui` crate

Mirrors the root crate's `similar_names = "deny"` (root `Cargo.toml`) — the `ui` crate has
its own `[lints.clippy]` table and doesn't inherit root's extended deny-list, so this never
applied to `ui/src` despite CI's `clippy` job running `--workspace`. The `ui` crate was already
clean, so this surfaced 0 violations. No behavior change.

chore(lint): enable `clippy::unnested_or_patterns` in the `ui` crate

Mirrors the root crate's `unnested_or_patterns = "deny"` (root `Cargo.toml`) — the `ui` crate has
its own `[lints.clippy]` table and doesn't inherit root's extended deny-list, so this never
applied to `ui/src` despite CI's `clippy` job running `--workspace`. The `ui` crate was already
clean, so this surfaced 0 violations. No behavior change.

fix(client): restore a working `client/` TypeScript build

`typescript` was bumped to `^7.0.2` (a pre-release/native-compiler major), but `openapi-typescript`
(which `generate:api` runs before every `typecheck`/`lint`/`test`/`build` script) declares a peer
dependency of `typescript: "^5.x"` and crashes immediately (`ts.factory` is `undefined`) under 7.x.
That single crash was tripping `pretypecheck`/`prelint`/`pretest` before those scripts ever ran,
so every PR's `client (typecheck + lint)` and `client (vitest)` CI jobs have been red since the
bump landed. `typescript` is pinned back to `^5.9.3`, the last version compatible with
`openapi-typescript`'s peer range.

With `generate:api` unblocked, `tsc --noEmit` surfaced two more breaks from unrelated dependency
bumps that had been landing behind the same crash: `cron-parser`'s v5 major dropped the
`parseExpression` named export in favor of the `CronExpressionParser.parse()` static method, and
`react-router-dom`'s v7 major removed the `future` prop entirely (its `v7_startTransition`/
`v7_relativeSplatPath` flags are now always-on defaults). Both call sites are updated to match.

`tsc --noEmit` is clean again. Out of scope for this patch (separate, pre-existing dependency-bump
regressions, unrelated to anything touched here): `eslint-plugin-react-hooks`'s new major flags
`react-hooks/set-state-in-effect` at several existing call sites, and `@vitejs/plugin-react`'s
6.x major wants `vite@^8` while the workspace still pins `vite@^6`, which crashes `vitest`'s config
load before any test runs.

fix(routines): serialize flag-creation's collision-check-then-write span to close a lost-update race

`create_flag` reads the routine's `flags/` directory to find a free `{type}-{timestamp}.md`
filename, then writes to it, with no synchronization between the check and the write. The HTTP and
MCP flag-creation handlers can be invoked concurrently on the multi-thread Tokio runtime, so two
overlapping calls for the same routine and flag type could both observe the same candidate filename
as free before either writes, and whichever write lands second would silently clobber the first —
directly contradicting `create_flag`'s own doc comment that "a flag never silently overwrites
another" (the same hazard class as the crontab, `machine.local.toml`, and default-tombstone
read-modify-write races fixed in issues #365, #1240, and #1243). `create_flag` now serializes
through a single `Mutex`, mirroring the existing `crontab_sync_lock`/`machine_toml_lock` pattern, so
concurrent flag creation can no longer clobber another in-flight flag.

fix: refuse to start on an unauthenticated non-loopback bind (`MOADIM_BIND_ADDR`) unless `MOADIM_ALLOW_REMOTE=1` is explicitly set. Closes #253.

test(ui,client): cover `healthBadge`/`healthBadgeClass` (and their Rust `RoutineHealth` counterparts) for every variant

`RoutineHealth::badge()`/`badge_class()` in `ui/src/routines/filter.rs` and their 1:1 TypeScript
port `healthBadge`/`healthBadgeClass` in `client/src/pages/routines/filter.ts` were the only
exported health-rendering functions with no test on either side — `priority()`/`healthPriority`
already had one, but the badge label and CSS class returned for each of the 7 `RoutineHealth`
variants were unverified. A typo or copy-paste duplicate (e.g. two variants sharing a CSS class,
or a mismatched label) would have shipped silently to the ROUTINES table's health badge. Both
sides now assert the exact rendered string per variant and that labels/classes stay unique across
variants, mirroring the existing `health_priority_order_dormant_most_urgent`/`healthPriority`
tests. No behavior change.

fix(routines): TZID-qualify the `.ics` feed's `DTSTART` against an embedded `VTIMEZONE`

`build_ical` (`GET /routines.ics`) emitted every `VEVENT` as a bare UTC instant with no embedded
`VTIMEZONE`, per issue #387. The fire times themselves were correct (evaluated in the host's local
zone, matching crontab semantics), but with no timezone identity in the feed, a subscribing
calendar rendered each event in *its own* default zone rather than the host's — a routine scheduled
`0 9 * * *` on a `UTC+3` host displayed at 06:00 to a subscriber whose calendar defaults to UTC.

When the host's zone can be named (`iana_time_zone`/`local_timezone`), the feed now emits one
`VTIMEZONE` component (a `STANDARD` sub-component pinned to the feed's current UTC offset) and
qualifies each `DTSTART` as `DTSTART;TZID=<zone>:<local-wall-clock>`, so a subscriber sees the
routine's actual configured local time regardless of their calendar's own default zone. `DTSTAMP`
stays UTC as RFC 5545 requires. When the zone can't be named, the feed falls back to the original
bare UTC-instant `DTSTART` with no `VTIMEZONE`, exactly as before.

Scope: this does not model DST transition rules (a full `STANDARD`/`DAYLIGHT` pair with recurrence
rules would need a timezone-database dependency the daemon doesn't have). A routine in a
DST-observing zone may display shifted by the DST delta once the host crosses a transition after
the feed was generated — tracked as a follow-up on issue #387, which also covers the full
DST-aware acceptance criteria.

fix(ui,client): "Unassigned" Machine filter facet now matches blank machine entries

The Machine filter's "Unassigned" option checked `machines.is_empty()` (Rust UI) /
`machines.length > 0` (React client) against the raw machine array, so a legacy routine created
before the `validate_machines` guard (#600) — one still carrying a blank/whitespace-only entry
like `[""]` — would never match "Unassigned", even though the Dormant status facet and the
Machine filter dropdown (#1221, #1223) already treat that same shape as "no real machine
assigned". Both sides now check `machines.iter().all(|m| m.trim().is_empty())` /
`machines.every((m) => m.trim() === "")`, matching the established convention.

fix(machine): serialize `machine.local.toml` writes to close a lost-update race

`set_machine` and `set_max_concurrent_runs_override` each read the whole `machine.local.toml`,
mutate one field, and write the whole struct back, with no synchronization between the two. `PUT
/machine` and `PUT /config/max-concurrent-runs` can be handled concurrently on the multi-thread
Tokio runtime, so two overlapping read-modify-write round trips could interleave and the later
write would silently drop whichever field the other request had just persisted (the same hazard
class as the crontab read-modify-write race fixed in issue #365). Both functions now serialize
through a single `Mutex`, mirroring the existing `crontab_sync_lock` pattern, so a concurrent
machine-name rename and concurrency-cap update can no longer clobber each other.

Split `src/cli/mod.rs`, `src/cli/tests.rs`, and `src/routines/ical_tests.rs` — all three had
grown past the 500-line `linecheck` gate (issue #974), which was failing on `main`. Extracted
the bind-address/loopback-policy logic into `src/cli/bind.rs` (with its tests in
`src/cli/bind_tests.rs`), and the `svc_ical`/`svc_ical_routine`/`build_ical` service-layer tests
into `src/routines/ical_service_tests.rs`. No behavior change.

test(client): cover `StatsBar`'s KPI tile counts and status-facet toggle

`client/src/pages/routines/StatsBar.tsx` — the KPI tile row above the routines table — had 0%
test coverage despite deriving eight non-trivial counts (total/enabled/disabled, due-soon,
snoozed, dormant, flagged, unregistered-agent) from the loaded routine list. Adds a test file
covering the derived counts, the `has-dormant`/`has-flags` conditional classes, the toggle-on/
toggle-off click behavior, and `aria-pressed` state. No production code changes.

feat(ui): add a RELIABILITY page ranking routines by success rate, active failure streaks, and
flakiness

Adds a new `/reliability` tab to the dashboard that ranks every routine by its most recent 20
finished runs (issue #1256): success rate, active pass/fail streak, and a flakiness signal
(≥40% adjacent-run status flips) distinct from steadily-failing routines. Ranked worst-first — an
active failure streak outranks a merely-low historical success rate. Reads the existing fleet-wide
`GET /api/v1/routines/runs` endpoint (already used by the Routines table's sparkline column); no
backend change.

## [1.3.1] - 2026-07-17

fix(sync): keep a slow crontab sync from stalling the async runtime

`sync_routines_to_crontab` shells out to `crontab -l`/`crontab -` synchronously from async REST/MCP
request handlers. Run inline on the multi-thread runtime, a slow or hung `crontab` binary could tie
up a worker thread and stall unrelated in-flight requests, including `/health` (#360). It now runs
via `tokio::task::block_in_place` whenever a multi-thread runtime is present, so the runtime can hand
off that thread's other scheduled work first; unit tests (which call the function directly with no
runtime, or under `#[tokio::test]`'s single-thread default) are unaffected and continue to run inline.

Bump `clap` (4.6.1 → 4.6.2) and `uuid` (1.23.5 → 1.24.0) to their latest compatible patch releases. No behavior change.

Bump `tokio` (1.52.3 → 1.52.4) and `console_log` (1.0.0 → 1.1.0) to their latest compatible releases. No behavior change.

feat(cli): add `moadim logs <id>` as a top-level shortcut for `moadim routines logs <id>`

The daemon has served a routine's newest run log over `GET /api/v1/routines/{id}/logs` since
`svc_logs()` landed, reachable from the CLI only via `moadim routines logs <id>`. `trigger`
already gets a bare top-level shortcut alongside its `routines trigger` form; `logs` did not
(issue #332). `moadim logs <id>` now mirrors that duality: same route, same exit-code
conventions (`0` on success including an empty not-yet-run log, non-zero on an unknown routine,
`3` when no daemon is reachable), documented in `--help` and shell completions.

Fix the React client (`client/`) silently dropping the absolute-timestamp hover tooltip that the
original Yew UI (`ui/`) shows next to every relative "N ago" time. `ui/src/cron_utils.rs`'s
`abstime` had no TypeScript port at all, so the "STARTED"/"UPDATED" cells in
`RecentRunsTable.tsx`/`RoutineRow.tsx` rendered no `title`, `RoutineHistory.tsx`'s run-row title
carried only the workbench name, and `RunHistorySparkline.tsx`'s per-tick tooltip omitted the
absolute time — all despite each file being documented as a "direct port" of its Rust
counterpart. Adds `abstime` to `client/src/lib/cronUtils.ts` (mirroring the Rust formatting and
its zero/out-of-range fallbacks) and wires it into the four call sites so hovering a relative time
in the React client shows the same wall-clock timestamp the Yew UI has always shown.

test(client): add Vitest coverage reporting (`pnpm --filter client test:coverage`). The `src/` and `ui/` crates already have a 100%-line-coverage CI gate, but `client/` (the newer React/TypeScript dashboard) had no coverage instrumentation at all. This adds a non-gating `v8` coverage report so gaps are visible; no threshold is enforced yet.

fix(ui): flag snoozed routines in the routines calendar's day-detail popover

The month grid already dims a routine's chip (amber, reduced opacity) when it is snoozed
(`snoozed_until` in the future, or `skip_runs` still pending), since that fire will be
silently skipped rather than actually run. The day-detail popover added alongside it listed
every enabled routine's fire time the same way regardless of snooze state, so a user opening
the popover lost that signal and could believe a snoozed routine's listed time will fire.
`day_fire_rows` now also reports each row's snoozed status (reusing the existing
`is_routine_snoozed` helper) and the popover renders a "SNOOZED" badge on those rows, matching
the styling and wording already used elsewhere (the routines table's health badge).

chore(lint): enable `clippy::format_collect` workspace-wide

Mirrors the existing `format_push_string = "deny"` lint (root `Cargo.toml`), which rejects
`.push_str(&format!(...))` in favor of writing straight into the buffer with `write!`. This
adds its sibling for the `.map(|x| format!(...)).collect::<String>()` shape, which has the same
throwaway-allocation problem but wasn't yet covered. Surfaced one violation in
`src/routines/service_trigger_tests.rs`, rewritten to fold a `writeln!` directly into the
accumulator instead of collecting a `Vec` of one-off `format!` strings. No behavior change.

chore(lint): enable `clippy::ignored_unit_patterns` in the `ui` crate

The `ui` crate has its own `[lints.clippy]` table and doesn't inherit the root crate's extended
deny-list, so `ignored_unit_patterns` (already `deny`d root-side, #1200) never applied to
`ui/src` despite CI's `clippy` job running `--workspace`. Enabling it surfaced 35 violations
across `main.rs`, `routines/page.rs`, `routines/hooks.rs`, `routines/bulk_actions.rs`,
`schedule_heatmap.rs`, `overview.rs`, `settings.rs`, `refresh.rs`, `machines.rs`, and
`routines/form.rs`: `use_effect_with((), move |_| ...)` hooks and `Callback::from(move |_: ()|
...)` handlers all matched the `()`-typed argument with `_`, discarding its type instead of
stating it explicitly. Applied via `cargo clippy --fix`, rewriting each `_`/`_: ()` to `()`. No
behavior change.

chore(lint): enable `clippy::large_stack_arrays` workspace-wide

Denies a local array literal over 512000 bytes — a value that size belongs on the heap
(`Vec`/`Box`), not the stack. A daemon process runs long-lived worker threads with a fixed,
comparatively small stack, so an oversized stack array is a latent stack-overflow risk that only
surfaces under the right call depth, unlike a heap allocation which fails safely. The codebase was
already clean, so this surfaced 0 violations. No behavior change.

chore(lint): enable `clippy::literal_string_with_formatting_args` workspace-wide

Denies a string literal that looks like a `format!`-family placeholder (`"{name}"`) sitting
outside a formatting macro — usually a leftover `format!`/`println!` argument that got moved
into a plain string and silently stopped interpolating. Surfaced 2 violations in
`routines/command.rs::substitute`, both intentional `String::replace` placeholder tokens rather
than formatting-macro arguments; annotated with a scoped `#[allow(reason = ...)]` explaining why.
No behavior change.

chore(lint): enable `clippy::many_single_char_names` workspace-wide

Rejects a scope with 4+ single-character bindings in play at once. Surfaced one violation:
`ui/src/routines/filter_tests.rs`'s `is_active_detects_each_facet` test had six (`q`, `s`, `a`,
`m`, `r`, `t`), one per `RoutineFilter` facet under test. Renamed to `query_filter`,
`status_filter`, `agent_filter`, `machine_filter`, `repo_filter`, `tag_filter`. No behavior change.

chore(lint): enable `clippy::needless_pass_by_value` in the `ui` crate

The `ui` crate has its own `[lints.clippy]` table and doesn't inherit the root crate's extended
deny-list, so `needless_pass_by_value` (already `deny`d root-side) never applied to `ui/src`
despite CI's `clippy` job running `--workspace`. Enabling it surfaced 5 violations in
`routines/actions.rs` and `routines/bulk_actions.rs`: `install_crud_handlers` and
`install_bulk_handlers` took their `state`/`toast`/`now` Yew handles by value but only ever
`.clone()`d them into closures, never consuming the outer parameter itself. Changed the
parameters to references (and updated the single call site in `routines/page.rs` to pass
borrows instead of pre-cloning), removing the needless ownership transfer. No behavior change.

chore(lint): enable `clippy::redundant_clone` in the `ui` crate

Mirrors the root crate's `redundant_clone = "deny"` (root `Cargo.toml`) — the `ui` crate has
its own `[lints.clippy]` table and doesn't inherit root's extended deny-list, so this never
applied to `ui/src` despite CI's `clippy` job running `--workspace`. The `ui` crate was already
clean, so this surfaced 0 violations. No behavior change.

chore(lint): enable `clippy::redundant_clone` workspace-wide. Fixes 31 violations across the `ui`
crate and the root crate's test suite: intermediate `let x = x.clone();` shadows built for a `move`
closure that turned out to be `x`'s last use, and a few `field.clone()` reads passed straight into
a constructor that never touched the original value again. Each is replaced with a direct move of
the original. No behavior change.

chore(lint): enable `clippy::ref_option` workspace-wide

Reject a `&Option<T>` parameter in favour of `Option<&T>` — the former forces every caller to
already own (or clone into) an `Option`, while the latter accepts a plain `&T` wrapped in `Some`
just as easily and is the idiomatic way to say "an optional borrow". Enabling it surfaced 1
violation in `ui/src/command_palette_match.rs`: `schedule_label` took `human: &Option<String>`
only to immediately match on it by reference. Changed the signature to `Option<&String>` and
updated its one call site and tests accordingly. No behavior change. The root `moadim` crate was
already clean, so `deny` there just locks it in.

chore(lint): enable `clippy::similar_names` workspace-wide

Rejects a binding whose name is a near-miss of another binding already in scope. Surfaced four
violations: `rmcp::model::ContentBlock::Text(txt) => txt.text.clone()`, repeated across the MCP
route tests, shadowed an existing local also named `text`. Renamed the match binding to `block`
in each spot. No behavior change.

chore(lint): enable `clippy::string_lit_as_bytes` in the `ui` crate. Mirrors the same lint
already enabled workspace-root-side (#1202) — the `ui` crate has its own `[lints.clippy]` table
with no `workspace = true` inheritance, so it was silently exempt despite `clippy --workspace`
covering it in CI. The `ui` crate was already clean, so no source changes were needed. No
behavior change.

chore(lint): enable `clippy::string_lit_as_bytes` in the root crate. Rewrites the two
`"...".as_bytes()` comparisons in `src/routes/http_settings_routes_tests.rs` to byte-string
literal slices (`&b"..."[..]`), stating "this is bytes" at the literal instead of via a runtime
conversion call. No behavior change.

chore(lint): enable `clippy::unnested_or_patterns` workspace-wide

Rejects an or-pattern repeated across multiple match arms/parameters instead of merged into a
single nested or-pattern, so duplicated arm bodies can't drift out of sync as arms are added or
reordered. The codebase was already clean, so no source changes were needed — `deny` just locks
that in.

chore(lint): enable clippy::use_self in the ui crate

Mirrors the root crate's `use_self = "deny"` (see `Cargo.toml`). The `ui` crate has its own
`[lints.clippy]` table and doesn't inherit root's extended deny-list, so this never applied
to `ui/src` despite CI's `clippy` job running `--workspace`. Fixed the 135 violations this
surfaced across `refresh.rs`, `routines/state.rs`, `routines/filter.rs`,
`overview_attention.rs`, and `schedule_heatmap_grid.rs` via `cargo clippy --fix`, replacing
enum/type name repetition (e.g. `RGroupBy::Status`) with `Self::Status` inside their own impl
blocks. No behavior change.

chore(lint): enable `clippy::ignored_unit_patterns` in the root crate. Rewrites the four
`tokio::select!` arms in `src/routes/http_listener.rs` that matched a `()`-typed future with `_`
to match `()` explicitly instead, so the pattern states its type rather than leaving the reader to
confirm `_` isn't silently discarding something meaningful. No behavior change.

chore(lint): enable `missing_docs` in the `ui` crate

The `ui` crate has its own `[lints]` table (no `workspace = true` inheritance), so root's
`missing_docs = "deny"` (in force since the project's early `[lints.rust]` table) never applied to
`ui/src` despite CI's `clippy`/`doc` jobs running `--workspace`. Enabling it surfaced 34 undocumented
public items in `main.rs`: the crate root doc, the `Route` enum and its variants, `ShellState` and
its fields, `ShellAction` and its variants/fields, and the `App`/`Nav` function components. Added
doc comments for each, and split `ShellState`/`ShellAction`/their `Reducible` impl out into a new
`shell_state.rs` module so `main.rs` stays under the workspace's 500-line-per-file convention. No
behavior change.

fix(ui,client): align the Routines "Dormant" status filter with the health badge/KPI definition of dormant. Both treated an empty `machines` list as dormant, but only the health badge/KPI (not the filter facet) also treated a list holding only blank/whitespace entries as dormant — so a routine could show a "DORMANT" badge and count toward the dormant KPI while filtering by `Status: Dormant` hid it. The filter now uses the same "no real machine assigned" check as the health/KPI logic in both the Yew UI and the React client.

Fix `highlightSegments` (`client/src/pages/routines/logSearch.ts`) silently dropping a log-search
match when it starts on or spans a character whose `toLowerCase()` expands to more than one code
point (e.g. Turkish `İ` → `i` + a combining dot above). The per-character lowercase array lost its
1:1 correspondence with the original text in that case, misaligning every subsequent window in the
sliding-window match. Now truncates each mapped entry to its first code point, mirroring the Rust
port's `c.to_lowercase().next().unwrap_or(c)` in `ui/src/log_viewer.rs`. No behavior change for
plain-ASCII queries.

fix(ui,client): omit blank machine entries from the Machine filter dropdown

`distinct_machines_r` (Yew UI) and `distinctMachines` (React client) collected every raw
`machines` string into the Machine facet's dropdown options, including blank/whitespace-only
entries. The API already rejects such entries on create/update (`validate_machines`, #600),
but routines written before that guard existed can still carry one, and `routineHealth`/
`routine_health` already treat it as "no real machine assigned" (dormant). Left unfiltered, a
legacy blank entry surfaced as a stray, unlabeled blank option in the dropdown, distinct from
"Any" and "Unassigned". Both helpers now skip blank/whitespace-only entries, matching the
health check's existing tolerance for this legacy data shape.

test(middlewares): cover the empty `x-request-id` header case in `logger`. The handler already falls back to a generated id when an inbound `x-request-id` is empty (`.filter(|header| !header.is_empty())`), but no test exercised that branch — a future edit removing the filter would silently start echoing back an empty correlation id. Test-only change, no behavior change.

feat(ui): day-detail popover on the routines calendar

Clicking a day number in the routines calendar month view now opens a popover listing that
day's fire times (`HH:MM`) per routine, sorted chronologically, each with a "▶ RUN" button
that triggers the routine immediately via the existing `POST /api/v1/routines/{id}/trigger`
endpoint. Closes the TODO.md item asking for this. Frontend-only: new pure `fires_on_day`
(`ui/src/schedule.rs`) and `day_fire_rows` (`ui/src/routines/calendar.rs`) helpers, both
host-tested; no backend or API changes.

## [1.3.0] - 2026-07-15

chore(deps): bump `openapi-fetch` from 0.13.8 to 0.17.0 (npm group) and regenerate `prebuilt-client.html` to match. No behavior change.

fix(routines): cap each workbench's `agent.log` to 32 MiB on the watchdog tick

`tmux pipe-pane -o` streams a session's raw pane output — every ANSI redraw
frame of a full-screen TUI agent included — into `agent.log` via an
unbounded, append-only `cat >>`. The `svc_logs`/`svc_run_log` read path
already bounds a single response to a 2 MiB tail (#280), but nothing bounded
the file's on-disk growth between TTL sweeps: a long-running or chatty
session could otherwise fill the disk before it was ever reaped (#268).

Adds `routines::cleanup::log_cap`, which truncates an oversized `agent.log`
in place to its last 32 MiB (prefixed with a marker noting how many bytes
were dropped) on the existing 30s watchdog tick, alongside the hung-session
kill check it already runs per workbench. Best-effort: an I/O failure for
one workbench is logged and does not abort the sweep for the rest.

Add a unit test for the client's `formatTtl` (workbench-retention duration formatter), the last pure-logic module in `client/src` without a matching `*.test.ts`. No behavior change — test-only.

Add a test for `cap_agent_log_to` propagating the `OpenOptions::open` error when the target path is a directory, closing an untested error branch in the watchdog's `agent.log` size cap. No behavior change — test-only.

Add host-side unit tests for the day timeline's `fire_times` (`ui/src/day_timeline.rs`), covering multi-fire schedules, the midnight-boundary seed, adjacent-day filtering, unparseable schedules, and the `MAX_FIRES` cap. This logic previously had no test module, unlike every other pure-logic file in the `ui` crate. No behavior change.

Dedupe the client's day-timeline fire-time math: `pages/routines/DayTimeline.tsx` had its own untested copy of the cron-to-fire-times logic (including the midnight-boundary seed trick) instead of the already-tested `fireTimesOnDay` used by the heatmap's day drill-down. Moved `fireTimesOnDay` into `lib/schedule.ts` as the single shared implementation (heatmap's `dayTimelineMath.ts` now re-exports it), pointed the routines page at it, and moved its tests to `schedule.test.ts`. No behavior change.

docs(cli): document that `moadim stop` does not kill in-flight routine sessions

`moadim stop` (and the UI STOP button / `POST /shutdown`) only stops the
daemon's own HTTP/MCP server. Routine agents run in a **detached** tmux
session (`tmux new-session -d`), independent of the daemon process, so an
in-flight run is never touched by a stop request — it keeps running (and can
keep opening PRs, filing issues, pushing commits, etc.) until it finishes on
its own or a later daemon start's watchdog/cleanup sweep reaps it (#320).

This behavior was previously undocumented, so `moadim stop` reporting
success could read as "everything stopped" when a routine agent was still
acting. Documents it in `moadim --help`, the `Command::Stop`/`stop()` doc
comments, `README.md`, `Architecture.md`, and `docs/moadim.1` — no behavior
change.

chore(lint): enable clippy::cast_lossless in the workspace

Adds `cast_lossless = "deny"` to both the root crate's and the `ui` crate's
`[lints.clippy]` tables, rejecting `as` casts that widen without loss (e.g.
`u32 as i64`) in favour of `From`/`Into`. An `as` cast stays silently legal
(and silently starts truncating) if the source or target type ever changes
size; `i64::from(x)` is the same widening but fails to compile the moment it
would no longer be lossless.

Fixed the single violation this surfaced, in `ui/src/routines/calendar.rs`'s
week-grid start calculation, replacing `... as i64` with
`i64::from(...)`. No behavior change.

chore(lint): enable clippy::derive_partial_eq_without_eq in the ui crate

Mirrors the root crate's `derive_partial_eq_without_eq = "deny"` (see `Cargo.toml`). The
`ui` crate has its own `[lints.clippy]` table and doesn't inherit root's extended deny-list,
so this never applied to `ui/src` despite CI's `clippy` job running `--workspace`. Fixed the
15 violations this surfaced by adding `Eq` alongside `PartialEq` on the affected structs and
enums, all of which are already field-for-field `Eq`-safe (no float fields).

chore(lint): enable clippy::or_fun_call in the ui crate

Mirrors the root crate's `or_fun_call = "deny"` (see `Cargo.toml`). The `ui` crate has its own
`[lints.clippy]` table and doesn't inherit root's extended deny-list, so this never applied to
`ui/src` despite CI's `clippy` job running `--workspace`. The `ui` crate is already clean under
it (zero violations), so `deny` locks that in. No behavior change.

chore(lint): enable clippy::or_fun_call in the root crate

Adds `or_fun_call = "deny"` to the root crate's `[lints.clippy]` table. It rejects a function
call passed directly as the fallback argument to `unwrap_or`/`ok_or`/`and`/`or`-style methods
(e.g. `opt.unwrap_or(expensive())`) in favour of the lazy `_else` form
(`opt.unwrap_or_else(expensive)`) — the eager form always evaluates the fallback, even on the
common path where the value is already present, doing needless work (or a needless allocation)
on every call.

The codebase is already clean under it (zero violations), so `deny` locks that in. No behavior
change.

feat(routines): expose the global concurrency cap through the UI/REST

`MOADIM_MAX_CONCURRENT_RUNS` was previously only configurable via the environment variable. A
new `GET`/`PUT /config/max-concurrent-runs` REST endpoint and a settings-page card now let the
cap be viewed and changed at runtime, persisted to `~/.config/moadim/machine.local.toml`
(gitignored, machine-local, same tier as the existing machine-name override). Precedence:
`MOADIM_MAX_CONCURRENT_RUNS` env var (ops/CI) > the persisted UI/REST override > unbounded.
Takes effect on the next trigger check — no restart required.

Opt the client's `BrowserRouter` (and the `MemoryRouter` used in `App.test.tsx`) into React Router's `v7_startTransition` and `v7_relativeSplatPath` future flags, silencing the two v7-upgrade warnings React Router logs on every render and test run. No behavior change.

fix(build): regenerate stale `prebuilt.html`

`prebuilt.html` last regenerated at #1122 no longer matches the compiled
`ui/` sources — merges since then (e.g. #1129, #1136) drifted the committed
bundle again, so `main` currently fails its own `prebuilt-html-fresh` CI
check and every open PR touching `ui/` inherits that failure regardless of
its own diff. Rebuilt via `cargo check` (trunk 0.21.14, matching the
workflow's pin) and committed the result. No source change.

Add a test for `svc_set_power_saving` returning 500/Internal when `write_routine` fails (read-only config dir), closing the last untested error branch in that handler. No behavior change — test-only.

test(ui): cover the UI's `humanize_bytes` byte-formatting helper

`routines::model::humanize_bytes` (used by the cleanup toast) mirrors the CLI's own
`humanize_bytes` (`src/cli/query.rs`, tested by `src/cli/cleanup_bytes_tests.rs`) byte-for-byte,
but had zero unit tests of its own — the `ui` crate isn't held to the root package's 100%
line-coverage floor, so this pure, deterministic function silently had no regression net despite
its CLI twin being fully covered. Adds the same edge cases the CLI test already exercises (sub-KB,
each unit boundary, MB-range rounding, and the u64::MAX TB cap) so a future edit that de-syncs the
two implementations' output fails a test instead of only showing up as a visual mismatch between
`moadim cleanup`'s CLI output and the UI's cleanup toast. No behavior change.

## [1.2.0] - 2026-07-13

feat(client): add a new React/TypeScript web client, served at `/client` alongside the existing `ui/`

A ground-up redesign of the web dashboard in React + TypeScript + Vite, with full feature parity
to the Yew `ui/` SPA (Overview, Routines, Heatmap, Settings). Built as a single self-contained
`dist/index.html` via `vite-plugin-singlefile` and embedded into the binary at compile time
(`src/build/client.rs`), mirroring `ui/`'s `prebuilt.html` pipeline. Served at `GET /client` (with
its own `/client/*` SPA fallback) purely additively — `ui/` at `/` is unchanged and still the
default. This is the first step of a planned rollout that will eventually retire `ui/`.

chore(lint): enable clippy::doc_markdown in the ui crate

Mirrors the root crate's `doc_markdown = "deny"` (see `Cargo.toml`). The `ui` crate has its
own `[lints.clippy]` table and doesn't inherit root's extended deny-list, so this never
applied to `ui/src` despite CI's `clippy` job running `--workspace`. Fixed the 6 violations
this surfaced across `cron_utils.rs`, `routines/banner.rs`, `routines/filter.rs`,
`routines/filter_bar.rs`, `routines/filter_tests.rs`, and `routines/hooks.rs` by wrapping
the flagged identifiers (`is_valid`, `DueSoon`, `schedule_description`, `NodeRef`) in
backticks. No behavior change.

chore(lint): enable clippy::if_not_else in the ui crate

Mirrors the root crate's `if_not_else = "deny"` (see `Cargo.toml`). The `ui` crate has its
own `[lints.clippy]` table and doesn't inherit root's extended deny-list, so this never
applied to `ui/src` despite CI's `clippy` job running `--workspace`. Fixed the 2 violations
this surfaced: `ui/src/header.rs`'s version-title span and `ui/src/overview.rs`'s attention
panel each wrote `if !x.is_empty() { A } else { B }`, rewritten as `if x.is_empty() { B }
else { A }` to drop the double-negation. No behavior change.

chore(lint): enable clippy::manual_let_else in the ui crate

Mirrors the root crate's `manual_let_else = "deny"` (see `Cargo.toml`). The `ui` crate has
its own `[lints.clippy]` table and doesn't inherit root's extended deny-list, so this never
applied to `ui/src` despite CI's `clippy` job running `--workspace`. Fixed the 1 violation
this surfaced in `ui/src/routines/state.rs::sort_routines`, rewriting a `match` whose only
non-binding arm returned early into `let Some(col) = col else { return routines };`. No
behavior change.

chore(lint): enable `clippy::needless_raw_string_hashes` in the ui crate

Adds `needless_raw_string_hashes = "deny"` to `ui/Cargo.toml`'s `[lints.clippy]` table. Mirrors
the root crate's lint (enabled in Cargo.toml), which never applied to `ui/src` since the `ui`
crate has its own `[lints.clippy]` table with no inheritance from the root. The `ui` crate is
already clean under it, so `deny` locks that in. No behavior change.

chore(lint): enable `clippy::semicolon_if_nothing_returned` in the `ui` crate

Adds `semicolon_if_nothing_returned = "deny"` to `ui/Cargo.toml`'s `[lints.clippy]` table,
matching the lint already denied workspace-root-side in `Cargo.toml` — the `ui` crate has its
own `[lints]` table (no `workspace = true` inheritance) so it silently escaped this despite CI's
`clippy` job running `--workspace`. Fixes the 10 violations this surfaced: `Callback::from`
closures in `main.rs`, `routines/actions.rs`, and `routines/page.rs` whose body was a bare
`spawn_local(...)`/`toast.emit(...)` call with no trailing semicolon, which read like the block
was returning that call's value even though the callback discards it. All fixes are a mechanical
added `;` — no behavior change. `prebuilt.html` is regenerated to match.

chore(lint): enable `clippy::single_match_else` in the `ui` crate

Adds `single_match_else = "deny"` to `ui/Cargo.toml`'s `[lints.clippy]` table, mirroring the
root crate's `single_match_else = "deny"` (`Cargo.toml`). The `ui` crate has its own
`[lints.clippy]` table with no `workspace = true` inheritance, so this lint (like several others
before it) silently never applied to `ui/src` despite CI's `clippy` job running `--workspace`.

`single_match_else` catches a `match` whose only non-wildcard arm destructures a single pattern,
with everything else falling to a catch-all arm — `if let ... else` says the same thing without
the unused generality of `match`, keeping a two-way branch as readable as a plain `if`.

The `ui` crate is already clean under this lint, so no code changes are needed — `deny` just
locks that state in. No behavior change.

fix(security): add missing `127.0.0.1:<port>` entry to the loopback `Host`/`Origin` allowlist

`allowed_hosts()` added `localhost:<port>` and `[::1]:<port>` alongside the bare bind address
when the daemon's bind address carries a port, but never added the equivalent
`127.0.0.1:<port>` entry — even though the bare `127.0.0.1` (no port) was already allowed. A
browser sending `Host: 127.0.0.1:<port>` (the common case for anyone loading the UI via the
raw IPv4 loopback address instead of `localhost`) was silently rejected with 403 by the
DNS-rebinding guard from issue #266, while the functionally identical `localhost:<port>` was
let through.

fix(cli): don't panic when writing a loopback HTTP request fails

`http_request_core` (`src/cli/system.rs`) used `.expect(...)` on `TcpStream::write_all`, even
though the very next line already tolerates a failed read on the same socket (a server that
closes the connection mid-request, e.g. while `moadim restart` is killing the old process).
Every caller (`status`, `stop`, `trigger`, `cleanup`, ...) already matches on this function's
`io::Result` to degrade gracefully to "moadim is not running" — the write failure just needs
to flow through the same `?` instead of panicking the CLI.

fix(client): edit-routine form no longer shows a blank prompt

`GET /routines` omits each routine's `prompt` by default (it's the largest field and rarely
needed in a listing). The React client's edit modal built its initial form values straight from
that cached list row, so the prompt textarea always opened empty and the Save button stayed
disabled until the user retyped the whole prompt. The edit modal now fetches the single routine
by id (`GET /routines/{id}`, which always includes the prompt) when it opens, showing a spinner
until it loads.

fix(ui): routines page stuck loading, never fetches on mount

`RoutinesPage`'s mount-time fetch went through `install_routines_loader`, a helper
that wraps `use_effect_with` and gets invoked as a bare statement in the component
body. That effect never actually fired at runtime, leaving `state.loading` permanently
true and the routine list empty even though the API responded fine. Inlined the effect
directly into `page.rs`, matching the pattern the working Overview page already uses,
and removed the now-dead `install_routines_loader` helper. Also added
`RequestCache::NoStore` to the routines list fetch so a stale cached empty response
can't mask this class of bug again.

fix(logs): snap truncated tail reads to the next line start

Prevents `--tail` reads that begin mid-line (because the read window doesn't align to
a line boundary) from emitting a partial first line. The read now skips ahead to the
next newline before returning output.

feat(routines): show local human-readable time alongside raw timestamps

Run-history API responses (`RunSummary`/`FleetRunSummary`), the daemon's structured JSON log,
and the UI's relative-time displays now also expose an absolute, human-readable local-time form
next to the existing raw Unix timestamp / relative "N ago" text, so timestamps are readable
without doing epoch math.

refactor(routes): move health HTTP + MCP endpoints into `routes/health`

Splits `src/routes/health/` into `mod.rs` (wiring), `logic.rs` (shared `HealthResponse` /
`DependencyHealth` types and the `build()` function), `http.rs` (the `GET /health` handler), and
`mcp.rs` (the MCP `health` tool, declared as a child module of `routes::mcp` so it keeps access to
`MoadimMcp`'s private state). The MCP tool now builds on the shared `logic::build()` instead of
re-deriving status/uptime/dependencies/version by hand, so the two surfaces can't drift.

No behavior change: same response fields on both `GET /health` and the MCP `health` tool.

refactor(routes): move restart HTTP + MCP endpoints into `routes/restart`

Follows the `routes/health/` / `routes/shutdown/` template (see
`src/routes/CONTRIBUTING.md`): splits the `POST /restart` handler and the MCP
`restart` tool into `src/routes/restart/` — `mod.rs` (wiring), `logic.rs`
(the shared `RestartResponse` type and a `build()` that spawns the detached
restart helper and builds the response), `http.rs`, and `mcp.rs` (declared as
a child module of `routes::mcp` so it keeps access to `MoadimMcp`'s private
state). Both surfaces now call the same `logic::build()` instead of each
spawning the helper and building the response separately.

No behavior change: same response fields, same log messages on each surface.

refactor(routes): move shutdown HTTP + MCP endpoints into `routes/shutdown`

Follows the `routes/health/` template (see `src/routes/CONTRIBUTING.md`):
splits the `POST /shutdown` handler and the MCP `shutdown` tool into
`src/routes/shutdown/` — `mod.rs` (wiring), `logic.rs` (the shared
`ShutdownResponse` type and a `build()` that fires the signal and builds the
response), `http.rs`, and `mcp.rs` (declared as a child module of
`routes::mcp` so it keeps access to `MoadimMcp`'s private state). Both
surfaces now call the same `logic::build()` instead of each notifying the
signal and building the response separately.

No behavior change: same response fields, same log messages on each surface.

fix(build): regenerate stale `prebuilt.html`

The committed `prebuilt.html` (last regenerated at #1092) no longer matches
the compiled `ui/` sources — every subsequent merge to `ui/src` recompiles
the embedded JS/WASM bytes, so the `prebuilt-html-fresh` CI job now fails on
any PR touching `ui/` even when that PR itself makes no visual change (see
#1119, #1120, #1113). Rebuilding via `cargo check` (which runs `build.rs` /
`trunk`) and committing the result restores a clean baseline so those and
future `ui/` PRs can pass the freshness check again. No source change.

docs(cli): document the `address` field in `moadim restart --json`'s output shape

`restart_json` (`src/cli/restart.rs`) has emitted `{"old":…,"new":…,"address":…}` since the
`address` field was added, but both the function's own doc comment and the README's `restart`
row still documented the older two-field shape (`{"old":N|null,"new":M}`), which the function's
own test (`restart_json_reports_old_new_pid_and_address`) already contradicted. Updated both to
match the real output. No behavior change.

fix(cli): rotate daemon.log on a daily tick, not just at spawn

`rotate_daemon_log_if_oversized` only rotated at detached-spawn time or on size, so a
long-lived daemon that stayed under the size cap and never restarted never rotated its log.
Renamed to `rotate_daemon_log_if_due` and added a 24h age-based trigger alongside the size
check, re-evaluated hourly via a new periodic task in `run_with_listener_until`.

docs(routes): add a template for logic/http/mcp endpoint folders

Adds `src/routes/CONTRIBUTING.md`, documenting the `mod.rs`/`logic.rs`/
`http.rs`/`mcp.rs` (+ `*_tests.rs` siblings) layout introduced by the
`routes/health/` refactor, so the next endpoint needing both a REST route
and an MCP tool over the same data has a copy-pasteable template — including
the `#[tool_router]`-splitting boilerplate (`vis = "pub(super)"`, the
parenthesized `Self::tool_router() + Self::<name>_tool_router()` router
combination, and the `__path_<name>` re-export utoipa needs) that isn't
obvious from reading `health/` alone. Root `CONTRIBUTING.md` now links to it
from the "Code conventions" section.

Docs only, no code change.

feat(routines): `MOADIM_MAX_CONCURRENT_RUNS` now defaults to unlimited (`0`)

The global routine concurrency cap (#335) previously defaulted to `4` and rejected `0` as an
"off" value, always falling back to the default instead. That was inconsistent with
`MOADIM_MAX_WORKBENCH_DISK_BYTES`'s "0 means unbounded" convention elsewhere in the daemon.
`0` (or unset) now means no cap is enforced; set `MOADIM_MAX_CONCURRENT_RUNS` to a positive
number to opt into bounding how many routine agent sessions may run at once.

Add a test for `write_routine` returning an error when `state.local.toml`'s path is occupied by a directory, closing the last untested error branch in `write_runtime_state` (`routine_storage.rs`). No behavior change — test-only.

feat(ui): saved views for the Routines page

Lets users save a named combination of filters/sort on the Routines page and switch back to it later.

Fix a manual `trigger_routine` that gets skipped (agent load failure, an oversized inline
prompt, the overlap guard, or the global concurrency cap) surfacing no reason anywhere a caller
could see (#1145). `spawn_routine_command`'s skip branches now also append the reason to a new
per-routine `skip.log`, and `svc_logs` (the `routine_logs` backend) falls back to it when no
workbench was spawned, instead of coming back indistinguishable from "never triggered".

test(middlewares): cover `allowed_hosts` when `MOADIM_BIND_ADDR` has no port

`allowed_hosts()` splits the configured bind address on `:` to derive a port and add
`localhost:<port>`/`[::1]:<port>` entries to the `Host`/`Origin` allowlist. The branch where
`MOADIM_BIND_ADDR` has no port (e.g. an operator setting it to a bare `0.0.0.0`) was never
exercised by a test — one of several gaps keeping the repo's 100%-line-coverage gate (`cargo
llvm-cov --fail-under-lines 100`, run in the pre-push hook) below 100% on `main`. Adds a test
asserting the port-suffixed entries are skipped in that case, bringing this file to 100%. No
behavior change.

Add a test for `PUT /config/user-prompt` returning 500 when the write itself fails (target path is a directory), closing the last untested error branch in that handler. No behavior change — test-only.

## [1.1.0] - 2026-07-12

chore(deps): bump `rmcp`/`rmcp-macros` to 2.2.0

Both stay within the `rmcp = "2.0.0"` (caret) requirement already declared in `Cargo.toml`, so
this is a `Cargo.lock`-only refresh — no manifest or code changes. The 2.1.0 -> 2.2.0 release
notes list only fixes (cancel-safe transport receive, refresh-token preservation, redirect
header-leak guard, unparsable-message handling, protocol version negotiation) and one addition
(rejecting auth servers lacking S256 PKCE support); no breaking changes. `cargo build`,
`cargo clippy --workspace --all-targets -- -D warnings`, and `cargo test --workspace` all pass
unchanged after the bump.

### Added

feat(cli): `moadim completions <shell>` prints a shell-completion script

Prints a completion script for `bash`, `zsh`, `fish`, `powershell`, or
`elvish` to stdout (e.g. `moadim completions zsh > _moadim`), covering the
lifecycle subcommands (`restart`, `stop`, `status`, `cleanup`, `trigger`,
`install`, `uninstall`, `machine`, `help`, `version`) and their `--json`/
`--quiet` flags. A missing or unrecognized shell prints a usage error to
stderr and exits non-zero, matching the rest of the CLI's error convention.

Generated via `clap_complete` from a small `clap::Command` built only for
this purpose — the CLI's existing hand-rolled argument parser is untouched
by this change (#307).

chore(lint): enable `clippy::allow_attributes_without_reason` in the `ui` crate

Adds `allow_attributes_without_reason = "deny"` to `ui/Cargo.toml`'s `[lints.clippy]` table,
matching the lint already denied workspace-root-side in `Cargo.toml` — the `ui` crate has its
own `[lints]` table (no `workspace = true` inheritance) so it silently escaped this despite
CI's `clippy` job running `--workspace`. There are no `#[allow(...)]` attributes anywhere in
`ui/src` today, so the `ui` crate is already clean under this lint; `deny` just locks that state
in and keeps any future suppression documented with a reason. No behavior change.

chore(lint): enable `clippy::case_sensitive_file_extension_comparisons` in the `ui` crate

Adds `case_sensitive_file_extension_comparisons = "deny"` to `ui/Cargo.toml`'s `[lints.clippy]`
table, matching the lint already denied workspace-root-side in `Cargo.toml` — the `ui` crate has
its own `[lints]` table (no `workspace = true` inheritance) so it silently escaped this despite
CI's `clippy` job running `--workspace`. The `ui` crate is already clean under this lint, so no
code changes are needed; `deny` just locks that state in. No behavior change.

chore(lint): enable `clippy::cloned_instead_of_copied` in the `ui` crate

Adds `cloned_instead_of_copied = "deny"` to `ui/Cargo.toml`'s `[lints.clippy]` table, matching the
lint already denied workspace-root-side in `Cargo.toml` — the `ui` crate has its own `[lints]`
table (no `workspace = true` inheritance) so it silently escaped this despite CI's `clippy` job
running `--workspace`. The `ui` crate is already clean under this lint, so no code changes are
needed; `deny` just locks that state in. No behavior change.

chore(lint): enable `clippy::dbg_macro`, `clippy::todo`, and `clippy::unimplemented` in the `ui` crate

The `ui` (Yew/WASM) crate has its own `[lints.clippy]` table with only `all = "deny"` — it does not inherit the workspace root's extended deny-list via `[lints] workspace = true`, so every lint enabled in root `Cargo.toml` (e.g. `dbg_macro`, `todo`, `unimplemented`) silently never applied to `ui/src` despite CI's `clippy` job running `--workspace`. A stray `dbg!()`, `todo!()`, or `unimplemented!()` left in the UI crate would ship straight into the release build and panic the running Yew app on that code path. Enables all three in `ui/Cargo.toml`; the `ui` crate already has zero violations, so no code changes are needed. No behavior change.

chore(lint): enable `clippy::explicit_into_iter_loop` in the `ui` crate

Adds `explicit_into_iter_loop = "deny"` to `ui/Cargo.toml`'s `[lints.clippy]` table, matching the
lint already denied workspace-root-side in `Cargo.toml` — the `ui` crate has its own `[lints]`
table (no `workspace = true` inheritance) so it silently escaped this despite CI's `clippy` job
running `--workspace`. Companion to the just-enabled `explicit_iter_loop`, this one catches the
`.into_iter()` (as opposed to `.iter()`/`.iter_mut()`) form of a redundant explicit iterator call
in a `for` loop. There are no such calls anywhere in `ui/src` today, so the `ui` crate is already
clean under this lint; `deny` just locks that state in. No behavior change.

chore(lint): enable `clippy::explicit_iter_loop` in the `ui` crate

Adds `explicit_iter_loop = "deny"` to `ui/Cargo.toml`'s `[lints.clippy]` table, matching the lint
already denied workspace-root-side in `Cargo.toml` — the `ui` crate has its own `[lints]` table
(no `workspace = true` inheritance) so it silently escaped this despite CI's `clippy` job running
`--workspace`. Unlike most sibling `ui`-crate lint-enablement PRs, this one wasn't a no-op: it
surfaced two real violations in `day_timeline.rs`, rewritten from `for it in props.items.iter()`
and `for b in buckets.iter_mut()` to `for it in &props.items` and `for b in &mut buckets`. No
behavior change.

chore(lint): enable `clippy::format_push_string` in the `ui` crate

Adds `format_push_string = "deny"` to `ui/Cargo.toml`'s `[lints.clippy]` table, mirroring the
root crate's `format_push_string = "deny"` (`Cargo.toml`). The `ui` crate has its own
`[lints.clippy]` table with no `workspace = true` inheritance, so this lint (like several others
before it) silently never applied to `ui/src` despite CI's `clippy` job running `--workspace`.

`format_push_string` catches `.push_str(&format!(...))`, which allocates a throwaway `String`
only to immediately copy its contents into the target and drop it — a real perf-adjacent gap, not
just a style one. `write!`/`writeln!` write straight into the existing buffer instead.

The `ui` crate is already clean under this lint, so no code changes are needed — `deny` just locks
that state in. No behavior change.

chore(lint): enable `clippy::items_after_statements` in the `ui` crate

Adds `items_after_statements = "deny"` to `ui/Cargo.toml`'s `[lints.clippy]` table, matching the
lint already denied workspace-root-side in `Cargo.toml` — the `ui` crate has its own `[lints]`
table (no `workspace = true` inheritance) so it silently escaped this despite CI's `clippy` job
running `--workspace`. The `ui` crate is already clean under this lint, so no code changes are
needed; `deny` just locks that state in. No behavior change.

chore(lint): enable `clippy::manual_string_new` in the `ui` crate

Adds `manual_string_new = "deny"` to `ui/Cargo.toml`'s `[lints.clippy]` table, matching the
lint already denied workspace-root-side in `Cargo.toml` — the `ui` crate has its own `[lints]`
table (no `workspace = true` inheritance) so it silently escaped this despite CI's `clippy` job
running `--workspace`. The `ui` crate is already clean under this lint, so no code changes are
needed; `deny` just locks that state in. No behavior change.

chore(lint): enable `clippy::mem_forget` in the `ui` crate

Adds `mem_forget = "deny"` to `ui/Cargo.toml`'s `[lints.clippy]` table, matching the lint already
denied workspace-root-side in `Cargo.toml` (#1121) — the `ui` crate has its own `[lints]` table
(no `workspace = true` inheritance) so it silently escaped this despite CI's `clippy` job running
`--workspace`. The `ui` crate is already clean under this lint, so no code changes are needed;
`deny` just locks that state in. No behavior change.

chore(lint): enable `clippy::mem_forget` in the root crate

Adds `mem_forget = "deny"` to `Cargo.toml`'s `[lints.clippy]` table. In a long-running daemon,
a `std::mem::forget`'d value's `Drop` impl never runs — for file handles, locks, and other RAII
guards that's an indefinitely leaked descriptor/lock rather than a one-off leak in a short-lived
program. The single existing use (a test that manually closes a file descriptor and must stop
`File::drop` from closing it again) gets a documented `#[allow(clippy::mem_forget, reason = ...)]`
so the intent stays explicit. No behavior change.

chore(lint): enable `clippy::needless_collect` in the `ui` crate

Adds `needless_collect = "deny"` to `ui/Cargo.toml`'s `[lints.clippy]` table, matching the lint
already denied workspace-root-side in `Cargo.toml` — the `ui` crate has its own `[lints]` table
(no `workspace = true` inheritance) so it silently escaped this despite CI's `clippy` job running
`--workspace`. The `ui` crate is already clean under this lint, so no code changes are needed;
`deny` just locks that state in. No behavior change.

chore(lint): enable `clippy::needless_raw_string_hashes` in the root crate

Adds `needless_raw_string_hashes = "deny"` to `Cargo.toml`'s `[lints.clippy]` table and drops the
unneeded `#` delimiters from the one raw string literal that had them
(`src/routines/command_system_prompt.rs`) — its body contains no unescaped `"`, so the hashes
were pure noise. No behavior change.

chore(lint): enable `clippy::redundant_else` in the `ui` crate

Adds `redundant_else = "deny"` to `ui/Cargo.toml`'s `[lints.clippy]` table, matching the lint
already denied workspace-root-side in `Cargo.toml` — the `ui` crate has its own `[lints]` table
(no `workspace = true` inheritance) so it silently escaped this despite CI's `clippy` job running
`--workspace`. The `ui` crate is already clean under this lint, so no code changes are needed;
`deny` just locks that state in. No behavior change.

chore(lint): enable `clippy::trivially_copy_pass_by_ref` in the `ui` crate

Adds `trivially_copy_pass_by_ref = "deny"` to `ui/Cargo.toml`'s `[lints.clippy]` table, matching
the lint already denied workspace-root-side in `Cargo.toml` — the `ui` crate has its own `[lints]`
table (no `workspace = true` inheritance) so it silently escaped this despite CI's `clippy` job
running `--workspace`. The `ui` crate is already clean under this lint, so no code changes are
needed; `deny` just locks that state in. No behavior change.

chore(lint): enable `clippy::unnecessary_debug_formatting` in the `ui` crate

Adds `unnecessary_debug_formatting = "deny"` to `ui/Cargo.toml`'s `[lints.clippy]` table, matching
the lint already denied workspace-root-side in `Cargo.toml` — the `ui` crate has its own `[lints]`
table (no `workspace = true` inheritance) so it silently escaped this despite CI's `clippy` job
running `--workspace`. The `ui` crate is already clean under this lint, so no code changes are
needed; `deny` just locks that state in. No behavior change.

chore(lint): enable `clippy::unreadable_literal` in the `ui` crate

Adds `unreadable_literal = "deny"` to `ui/Cargo.toml`'s `[lints.clippy]` table, matching the lint
already denied workspace-root-side in `Cargo.toml` — the `ui` crate has its own `[lints]` table (no
`workspace = true` inheritance) so it silently escaped this despite CI's `clippy` job running
`--workspace`. The `ui` crate is already clean under this lint, so no code changes are needed;
`deny` just locks that state in. No behavior change.

fix(service): `install`/`uninstall` return an error instead of panicking on macOS when `$HOME` is undeterminable

The macOS launchd backend's `install()` and `uninstall()` both `.expect()`-ed the home directory
lookup, crashing the whole process with a panic if the home directory couldn't be resolved (e.g.
`$HOME` unset and no passwd entry, such as some minimal service/CI contexts). `plist_path_from_home`
already turns that condition into a proper `anyhow::Error` — the callers just weren't using it. Now
both functions propagate the error via `?`, matching the Linux systemd backend's `install()`/
`uninstall()`, which already propagate their equivalent `unit_path()` lookup the same way. No
behavior change on the happy path.

fix(server): skip rewriting the on-disk openapi spec when it hasn't changed

`write_openapi_spec` already skipped the write when `apis/`'s parent directory is
absent (the installed-binary case). It still rewrote the file unconditionally on
every dev startup even when the freshly generated spec was byte-for-byte identical
to what's already on disk, needlessly bumping the committed file's mtime. It now
compares against the existing contents first and skips the write when unchanged (#319).

### Added

feat(ui): inline run-history sparkline column in the Routines table

Each row now shows a compact strip of ticks for its last ~10 runs (green =
success, red = failed, pulsing amber = running, gray = unknown/no data),
between LAST FIRE and AGENT — an at-a-glance pass/fail trend without opening
the routine's HISTORY page, mirroring the "pipeline graph" pattern common to
CI dashboards. Reuses the existing fleet-wide `GET /routines/runs` endpoint
(already backing the Overview page's recent-runs panel), fetched once and
grouped client-side by routine — no new API calls per row (#1103).

test(cli): cover `restart --quiet` skipping the endpoint-hint block

`restart(json, quiet)` never had a test exercising `quiet=true` — every existing case (parse
tests aside) only called `restart(_, false)`, so the `if !quiet { report_endpoints(); }` branch
that suppresses the UI/stop/logs hints was unverified behavior. Adds
`restart_quiet_skips_endpoint_hints_when_none_running`, mirroring the existing
`restart_json_skips_human_text_when_none_running` case. Test-only, no behavior change.

## [1.0.1] - 2026-07-09

fix(ci): stop `publish.yml`/`release.yml` from racing their own redundant `test.yml` re-run on the automated release path (#1099)

Both workflows gated a redundant `lint`/`test` re-run on `github.event_name == 'push'`, meant to skip it when called from `auto-release.yml` on a verified version bump. A nested `workflow_call` inherits `github.event_name` from the chain's originating event, so it read `push` there too and the guard never actually skipped anything — every version bump ran three concurrent `test.yml` calls sharing one `test-<ref>` concurrency group with `cancel-in-progress: true`, and `auto-release.yml`'s `publish`/`release` jobs routinely lost that race, silently skipping the crates.io publish and/or GitHub Release step (reproduced on `v1.0.0`). The guard now keys off `inputs.tag == ''`, which reliably distinguishes the two paths regardless of what `github.event_name` reports.

## [1.0.0] - 2026-07-09

chore(lint): enable `clippy::map_unwrap_or`

Adds `map_unwrap_or = "deny"` to the workspace root `Cargo.toml`'s `[lints.clippy]` table, rejecting `.map(f).unwrap_or(_)`/`.map(f).unwrap_or_else(_)` in favour of the idiomatic `map_or`/`map_or_else`/`is_ok_and` single-combinator form. Fixes the violations this surfaced across `src/` at the time (`routes/mcp.rs`, `routine_storage.rs`, `routines/cleanup/mod.rs`, `routines/cleanup/session.rs`, `utils/time.rs`). No behavior change. (#524)

fix(build): inline the compiled CSS into `prebuilt.html`, not just JS/WASM

`src/build/ui.rs`'s `inline_into_html` folded trunk's compiled JS and WASM
into a single self-contained `index.html`, but left the CSS as an external
`<link rel="stylesheet" href="./styles-<hash>.css">` — a file that never
gets embedded or shipped alongside `prebuilt.html` (only the HTML itself is
copied to the package root and committed). Every `cargo install moadim`
user hit this: the server's catch-all route serves `index.html` for that
missing CSS request, the browser gets `text/html` back for a `.css`
request, and strict MIME checking refuses to apply it — the control panel
rendered completely unstyled. Present since CSS inlining was never added
alongside JS/WASM inlining (reproduces on the v0.26.0 tag too); this is the
first release to carry the fix.

`find_dist_assets` now also locates the `.css` file in trunk's `dist/`,
and `assemble_html` inlines it into a `<style>` block in `<head>` the same
way the WASM bytes are inlined into the boot script, so the served bundle
makes zero external asset requests.

fix(routines): verify the Claude trust-dialog pre-seed actually persisted before launching

The `claude` agent's `setup` step pre-seeds `~/.claude.json` so a headless routine run
never blocks on Claude Code's "Do you trust this folder?" dialog. Live runs were found
parked at that exact dialog for hours — reaped only by the ~1h watchdog, with an empty
log — because the write had silently not taken effect for that workbench. The setup
script now reads `~/.claude.json` back after writing and asserts the seeded entry is
actually there; a failed assertion makes the script exit non-zero, which the launcher's
existing `{setup}; } || { ...; exit 1; }` guard turns into an immediate, diagnosable
"agent setup failed" abort instead of a silent multi-hour hang.

feat(read): reload the routine store from disk on every GET (#774)

The daemon used to load `~/.config/moadim/routines/` once at startup into an in-memory cache; every `GET` (HTTP `/routines`, `/routines/{id}`, `/routines.ics`, and the equivalent MCP tools) served that stale snapshot, so config edits pulled into the directory — e.g. a routine's `machines` targeting list changing via `git pull` — stayed invisible until a daemon restart. `svc_list`/`svc_get`/the iCal feed now re-scan the on-disk routines directory and refresh the store before serving each request; disk is already the source of truth (every mutation persists before returning), so the reload-on-read loses no state, and the scheduler-written `last_scheduled_trigger_at` log is read back on every reload so it survives the refresh.

## [0.27.0] - 2026-07-07

### Added

The build now generates `schemas/routine.schema.json` and `schemas/routine.example.toml` (a JSON Schema + annotated example for the on-disk `routine.toml`), giving routine configs editor validation/completion. The schema documents every field the daemon writes — including the legacy, read-only `last_(manual_)trigger_at` keys now kept in the `state.local.toml` sidecar — and is regenerated on every build from the `RoutineToml` shape (#388).

### Fixed

A manual ("run now") routine trigger no longer overwrites `last_scheduled_trigger_at`. The launch script it shares with the scheduled path unconditionally appended the fire time to the routine's `scheduled.log`, so every manual run masqueraded as a scheduled fire and clobbered the real last-scheduled time. `build_routine_command` now takes a `TriggerSource` (`Scheduled`/`Manual`); only a genuine scheduled fire appends to `scheduled.log`, while a manual trigger launches the agent but leaves it untouched, staying tracked solely via `last_manual_trigger_at` (#478).

Move the `moadim` CLI's parsing/lifecycle files (`cli.rs`, `cli_query.rs`, `cli_system.rs`, `cli_restart.rs`, and their `*_tests.rs` siblings — 14 files total) into a `src/cli/` folder, per the TODO.md request to colocate all CLI-command files instead of leaving them as flat, prefix-named siblings in `src/`. Pure file move: module paths, `#[path = ...]` attributes, and one `include_str!("../README.md")` → `("../../README.md")` were updated to match the new depth; no behavior change.

chore(lint): enable `clippy::case_sensitive_file_extension_comparisons`

Locks in the codebase's existing (near-)zero-violation state for `clippy::case_sensitive_file_extension_comparisons`, so a future case-sensitive `.ends_with(".ext")` file-extension check fails CI instead of silently disagreeing with the case-insensitive filesystems (macOS, Windows) this daemon runs on. Fixes the two existing violations in `src/routines/flags.rs` and its tests by switching `is_safe_flag_filename` (and the test asserting its shape) from `ends_with(".md")` to `Path::extension()` compared with `eq_ignore_ascii_case`. No behavior change on case-sensitive filesystems (Linux); on case-insensitive ones, a flag file named e.g. `bug-123.MD` is now correctly recognized instead of rejected.

chore(lint): enable `clippy::cloned_instead_of_copied`

Locks in the codebase's existing zero-violation state for `clippy::cloned_instead_of_copied`, so a future `.cloned()` call on an iterator/`Option` of a `Copy` type fails CI instead of shipping a needlessly indirect clone where `.copied()` says the same thing more directly. No behavior change.

chore(lint): enable `clippy::format_push_string`

Fixes the 3 existing violations in `compose_prompt` (`src/routines/command.rs`), which built a repository/flag line with `format!` only to immediately copy it into the routine's prompt body via `push_str` — an unnecessary throwaway `String` allocation per line. Switches those to `write!`/`writeln!` directly into the existing buffer, and enables `format_push_string = "deny"` to lock in the zero-violation state going forward. No behavior change.

chore(lint): enable `clippy::items_after_statements`

Fixes the 3 existing violations — a `use` mid-function in `read_log_tail_of_len` (`src/routines/service_log_tail.rs`), a `use` mid-test in `service_overlap_guard_tests.rs`, and a `const` mid-test in `routines_sync_tests.rs` — by hoisting each item to the top of its block. Enables `items_after_statements = "deny"` to lock in the zero-violation state going forward. No behavior change.

### Changed

chore(lint): enable `clippy::map_unwrap_or` in the `ui` crate

Adds `map_unwrap_or = "deny"` to `ui/Cargo.toml`'s `[lints.clippy]` table, matching the lint already
denied workspace-root-side in `Cargo.toml` — the `ui` crate has its own `[lints]` table (no
`workspace = true` inheritance) so it silently escaped this despite CI's `clippy` job running
`--workspace`. Fixes the 7 violations this surfaced across `log_viewer.rs`,
`overview_recent_runs.rs`, `routines/form.rs`, `routines/history.rs`, and `routines/hooks.rs`,
rewriting each `.map(f).unwrap_or(_)`/`.map(f).unwrap_or_else(_)` into the idiomatic
`map_or`/`map_or_else`/`is_some_and` single-combinator form. No behavior change.

### Changed

chore(lint): enable `clippy::match_same_arms` in the `ui` crate

Adds `match_same_arms = "deny"` to `ui/Cargo.toml`'s `[lints.clippy]` table, matching the lint
already denied workspace-root-side in `Cargo.toml` — the `ui` crate has its own `[lints]` table (no
`workspace = true` inheritance) so it silently escaped this despite CI's `clippy` job running
`--workspace`. Fixes the 5 violations this surfaced in `routines/filter.rs`: each facet's explicit
`Facet::All`/`Facet::Any` match arm did nothing that the trailing wildcard arm didn't already do, so
they're removed as dead code. No behavior change.

chore(lint): enable `clippy::needless_collect`

Locks in the codebase's existing zero-violation state for `clippy::needless_collect`, so a future PR that collects an iterator into a `Vec`/collection only to immediately re-iterate it (or check its length/emptiness) fails CI instead of shipping needless allocation overhead. No behavior change.

chore(lint): enable `clippy::needless_pass_by_ref_mut` in the `ui` crate

Mirrors the root crate's existing `needless_pass_by_ref_mut = "deny"` into `ui/Cargo.toml`'s own `[lints.clippy]` table, which doesn't inherit root's extended deny-list. Locks in the crate's existing zero-violation state so a future stale `&mut` parameter fails CI instead of overstating what the function does to its caller. No behavior change.

chore(lint): enable `clippy::redundant_closure_for_method_calls` in the `ui` crate

The `ui` (Yew/WASM) crate has its own `[lints.clippy]` table with only `all = "deny"` — it does not inherit the workspace root's extended deny-list via `[lints] workspace = true`, so every lint enabled in root `Cargo.toml` (e.g. `redundant_closure_for_method_calls`, already denied since #549) silently never applied to `ui/src` despite CI's `clippy` job running `--workspace`. Enables it in `ui/Cargo.toml` and fixes the 3 violations it surfaces. No behavior change.

chore(lint): enable `clippy::single_match_else`

Fixes the 2 existing violations — `machine::run`'s `Some("set")` arm and `defaults::ensure_default_routines`'s existing-routine lookup — each a `match` destructuring a single pattern with the rest falling to a catch-all arm. Switches both to `if let ... else`, and enables `single_match_else = "deny"` to lock in the zero-violation state going forward. No behavior change.

Enable `clippy::trivially_copy_pass_by_ref` (deny) to reject `&T` parameters where `T` is a small `Copy` type — pass-by-value is at least as cheap and states the callee doesn't need the caller's own reference. Codebase was already compliant, no code changes needed.

Deny `clippy::uninlined_format_args` in the `ui` crate, matching the root crate's lint config. The `ui` crate has its own `[lints.clippy]` table that doesn't inherit the root's deny-list, so this lint never applied to `ui/src` despite CI's `clippy` job running `--workspace`. The crate is already clean, so this locks it in.

chore(lint): enable `clippy::unreadable_literal`

Locks in the codebase's existing (near-)zero-violation state for `clippy::unreadable_literal`, so a future long integer literal without `_` digit-group separators fails CI instead of shipping a number that's hard to judge the magnitude of at a glance. Fixes the one existing violation (`424242` → `424_242` in `restart_tests.rs`). No behavior change.

chore(lint): enable `clippy::unused_async` in the `ui` crate

The `ui` (Yew/WASM) crate has its own `[lints.clippy]` table with only `all = "deny"` — it does not inherit the workspace root's extended deny-list via `[lints] workspace = true`, so every lint enabled in root `Cargo.toml` (e.g. `unused_async`) silently never applied to `ui/src` despite CI's `clippy` job running `--workspace`. Enables it in `ui/Cargo.toml`; the `ui` crate already has zero violations, so no code changes are needed. No behavior change.

chore(lint): enable `clippy::unused_self` in the `ui` crate

The `ui` (Yew/WASM) crate has its own `[lints.clippy]` table with only `all = "deny"` — it does not inherit the workspace root's extended deny-list via `[lints] workspace = true`, so every lint enabled in root `Cargo.toml` (e.g. `unused_self`, already denied since #1067) silently never applied to `ui/src` despite CI's `clippy` job running `--workspace`. Enables it in `ui/Cargo.toml`; the `ui` crate already has zero violations, so no code changes are needed. No behavior change.

chore(lint): enable `clippy::unused_self`

Enables `unused_self = "deny"` to reject a `&self` method that never reads `self`. The 3 existing
violations — `list_agents`, `get_lock_status`, and `restart` in `src/routes/mcp.rs` — are `#[tool_router]`
MCP tool handlers whose `&self` receiver is dictated by the framework's uniform `self.method(...)`
dispatch, not by need, so each gets a documented `#[allow(clippy::unused_self, reason = "...")]` instead
of a signature change. No behavior change.

chore(lint): enable `clippy::unwrap_used` in the `ui` crate

The root crate denies `clippy::unwrap_used` in production code so a panic can't kill the
long-running daemon process; `ui/Cargo.toml` never inherited it, so the same class of
unhandled panic could ship in the dashboard UI unchecked. Adds the lint to `ui/Cargo.toml`
and fixes the one existing violation in `shell_dialogs.rs` (a `serde_json::to_string` call
that cannot actually fail, now an `.expect()` with a reason instead of a bare `.unwrap()`).
No behavior change.

chore(lint): enable `clippy::wildcard_imports` in the `ui` crate

The `ui` (Yew/WASM) crate has its own `[lints.clippy]` table with only `all = "deny"` — it does not inherit the workspace root's extended deny-list via `[lints] workspace = true`, so every lint enabled in root `Cargo.toml` (e.g. `wildcard_imports`) silently never applied to `ui/src` despite CI's `clippy` job running `--workspace`. Enables it in `ui/Cargo.toml`; the `ui` crate already has zero violations, so no code changes are needed. No behavior change.

Move `POST /routines/{id}/trigger`, `/scheduled-trigger`, and `/routines/cleanup` off the async worker thread. These handlers call `svc_trigger`/`svc_trigger_scheduled`/`svc_cleanup`, which shell out to `tmux`(1) and do blocking filesystem I/O; they previously ran inline on the Tokio worker thread instead of `spawn_blocking`, unlike the sibling create/update/delete/lock/unlock handlers. A hung `tmux` call (or a `*/N` scheduled-trigger herd) could stall unrelated requests such as `GET /health`.

fix(routines): guard `write_routine` against a stale on-disk slug collision (#188)

Two distinct routine titles can slugify to the same folder name (e.g. `"Update deps!"` and `"Update deps?"` both become `update-deps`). The in-memory create/update handlers already reject that when both routines are loaded, but a slug could also collide with a stale `routine.toml` left on disk by something outside the in-memory store (e.g. a directory `remove_routine_dir` failed to clean up) — and `write_routine` would silently overwrite it, including the wrong `prompt.md` a scheduled run then executes. `write_routine` now checks the target slug's existing `routine.toml` id before writing and refuses to overwrite a different routine's files, surfaced as a 409 Conflict instead of a 500 at the `create`/`update` API handlers.

### Added

A shared cron minute (e.g. `*/5 * * * *`, `0 * * * *`) could launch an unbounded thundering herd of agent sessions: each routine fire spawns its own detached tmux session with no cap on how many may be alive across *all* routines at once — distinct from the existing per-routine overlap guard, which only stops one routine from stacking on its own still-running fire. `MOADIM_MAX_CONCURRENT_RUNS` (default `4`) now caps the number of concurrently-running routine agent sessions; a fire that would exceed it is skipped (logged, not queued) and picked up again on its next scheduled tick. The live count is derived from actual tmux session liveness, not an in-memory counter, so it stays correct across a daemon crash/restart.

fix(routines): iCalendar feed now skips power-saving and snoozed fires

`GET /routines.ics` only excluded disabled routines and unparseable schedules,
so a routine in power-saving mode, snoozed via `snoozed_until`, or with
`skip_runs` pending still advertised upcoming fire times that
`svc_trigger_scheduled` would actually refuse to spawn — a subscribed calendar
lied about what would run. The feed now filters/skips those fires the same way
the trigger path does, so `.ics` subscribers never see a run that will
silently no-op.

docs(routines): note the `> 0` requirement on `ttl_secs`/`max_runtime_secs`

`svc_create`/`svc_update` already reject `ttl_secs: 0` and `max_runtime_secs: 0`
with `400 Bad Request` (#239), but the REST/MCP field docs (and the generated
OpenAPI spec) never said so, leaving the constraint undiscoverable to callers
until they hit the error. Documents the minimum on `Routine`,
`CreateRoutineRequest`, `UpdateRoutineRequest`, and the MCP `update_routine`
input, closing the last unchecked box on #233.

fix(routines): stat `agent.log` once when reading its tail with metadata

`read_log_tail_with_meta` (backing the MCP `routine_logs` tool and the HTTP
logs route) stated the log file twice: once for `total_bytes`/`truncated`,
then again inside `read_log_tail` to size the actual read. For a log still
being appended to by a live `tmux pipe-pane` capture, the file could grow
between those two stats, so the reported `total_bytes`/`truncated` could
describe a different moment in time than the `content` actually returned.
Both callers now share a single stat, so the metadata always matches the
content it describes.

fix(routines): manual triggers no longer clobber `last_scheduled_trigger_at`

A manual ("run now") routine trigger no longer overwrites `last_scheduled_trigger_at`. The launch script it shares with the scheduled path unconditionally appended the fire time to the routine's `scheduled.log`, so every manual run masqueraded as a scheduled fire and clobbered the real last-scheduled time. `build_routine_command` now takes a `TriggerSource` (`Scheduled`/`Manual`); only a genuine scheduled fire appends to `scheduled.log`, while a manual trigger launches the agent but leaves it untouched, staying tracked solely via `last_manual_trigger_at` (#478).

Add `GET /routines/{id}/prompt-preview` (and the matching `preview_routine_prompt` MCP tool) to return the exact composed prompt body a routine's run would receive, computed in-memory with no workbench, `prompt.md` write, or agent launch. Lets operators verify repo clone bullets and prompt composition before a scheduled or manual run consumes a workbench.

fix(routines): reconcile pristine built-in agent configs on startup

Built-in agent configs (`claude.toml`, `codex.toml`, `hermes.toml`) were only seeded when absent and never refreshed afterward — a shipped fix to a default agent config never reached an existing install. Startup now rewrites an existing config that is still pristine (unedited since the daemon wrote it) but stale, using a fingerprint header to distinguish pristine-but-stale from user-edited, mirroring the existing routine-defaults reconciliation. A user-edited config, or one with no managed header, is left untouched.

fix(routines): rename the compiled-prompt sidecar to prompt.compiled.local.md

`prompts/prompt.compiled.md` is fully derived from `prompt.pure.md` + `routine.toml`
and rewritten on every save, so it should never be tracked — but relying on an
explicit `.gitignore` entry (added in #1050) only stopped *new* writes from being
tracked; it did nothing for installs where the file had already been `git add`-ed
before that fix landed. Renamed it to `prompt.compiled.local.md` so it matches the
`*.local.*` pattern the same way `state.local.toml` does, and dropped the now-redundant
explicit `.gitignore` entry (#1046).

A new startup migration (`migrate_compiled_prompt_filename`) renames the file on disk
for existing routines. This does not touch git history or the index — the daemon has
no git integration — so an install with `prompt.compiled.md` already committed will
still need a manual `git rm --cached prompts/prompt.compiled.md` (or just let the next
commit record the rename) after upgrading.

fix(routines): rotate a routine's `runs.log` instead of letting it grow forever

The reaper appends one durable [`PersistedRun`] record to a routine's
`runs.log` right before reaping its workbench, with no other trim point —
the same unbounded-growth shape already fixed for `daemon.log` (#316), just
scoped per routine instead of per daemon. A long-lived, frequently-firing
routine's history would otherwise grow without bound. `append_persisted_run`
now rotates `runs.log` to a sibling `runs.log.1` (replacing any previous one)
once it exceeds 1 MiB, mirroring `DAEMON_LOG_MAX_BYTES`'s rotate-and-replace
approach.

feat(build): generate `routine.toml` JSON Schema + example

Generates `schemas/routine.schema.json` and `schemas/routine.example.toml` at build time from the
`RoutineToml` shape, mirroring the existing `job.schema.json` generation. Example TOMLs can reference
the schema via `#:schema ./routine.schema.json` for editor validation.

## [0.26.0] - 2026-07-07

### Fixed

Auto-refresh the routine LOGS view on the same operator-chosen cadence already used by the routines list (via the shared `AUTO` interval control), instead of only reloading once on mount. Previously, a workbench reaped by the periodic background cleanup sweep while a run's LOGS page was open left stale, already-deleted output on screen until the operator remembered to click the manual "↻" button (#357).

test(ui): cover `parse_cron`/`describe_cron_live` in `cron_utils.rs`

`cron_utils.rs`'s field-count normalization (5-field passthrough, 6-field
with seconds, 7-field seconds+year stripping, `@keyword`, and invalid input)
and `describe_cron_live`'s validity/description pairing had no tests at
all, unlike the sibling `schedule.rs`/`schedule_heatmap.rs` pure-logic
modules which both have dedicated `*_tests.rs` files. Added
`cron_utils_tests.rs` following that same host-tested convention.
`reltime` is left untested — it calls `js_sys::Date::now()` and needs a
wasm/DOM host, mirroring the pure/DOM split already documented in
`refresh.rs`.

No behavior change — regression tests only.

### Fixed

Each routine's seeded `.gitignore` now also ignores `prompts/prompt.compiled.md` — the composed prompt is fully derived from `prompt.pure.md` + `routine.toml` and rewritten on every save, so it was getting tracked/committed even though it carries no information of its own (#1046). The pattern is reconciled into existing `.gitignore` files (not just newly created ones) the next time the daemon starts, alongside any other patterns a user has added.

`host_validation` middleware: a present-but-non-UTF-8 `Host` or `Origin` header is now rejected with `403` instead of being silently treated the same as a missing header. `HeaderValue::to_str()` only rejects non-ASCII bytes, which no legitimate client ever sends in these headers, so falling through to "allow" on that error let an attacker bypass the DNS-rebinding/cross-origin allowlist entirely by sending garbage bytes in `Host`/`Origin`. Adds regression tests for both headers.

Report `total_bytes` + `truncated` alongside log tail content in the logs MCP tool, so callers can tell a full log from a truncated window (#280).

feat(ui): add TTL preset row (1h/1d/7d/30d) to the routine form

The WORKBENCH TTL input required typing a raw second count from memory,
unlike the SCHEDULE field which already has one-click cron presets. The
routine create/edit form now has a matching preset row under the TTL
input — 1h/1d/7d/30d buttons that set the field to the corresponding
second count — mirroring the cron schedule presets' styling and behavior.

feat(routines): show a humanized retention countdown per finished run in the
run-history view

`RunSummary` now carries `retention_expires_at` (finish time + the routine's
effective TTL) for runs whose workbench is still on disk. The HISTORY page
renders it as a `RETENTION` column ("expires in 12m" / "expired"), so users
can see how long a finished run's log stays before cleanup reaps it, instead
of guessing from the TTL alone (#477).

Split `src/routines/service_trigger.rs` (→ `service_run_files.rs`) and `src/cli.rs` (→ `cli_restart.rs`) to satisfy the 500-line pre-push gate, which two independently-passing PRs had combined to exceed (`linecheck` isn't a required status check on the branch ruleset, so neither merge was blocked by it). No behavior change.

### Fixed

Warn at startup when the server binds to a non-loopback address. The REST/MCP API has no authentication (#504), so exposing it beyond `127.0.0.1`/`::1` grants anyone who can reach that address unauthenticated routine CRUD; the daemon now logs a loud warning at launch, matching the existing tmux/python3 startup checks, instead of leaving this risk silent.

Add `MOADIM_MAX_WORKBENCH_DISK_BYTES`, an optional total-disk ceiling for `~/.moadim/workbenches/`. The existing TTL sweep only reaps a workbench once it is old enough, so a handful of concurrent large runs (e.g. big repo clones) could exhaust the disk before any TTL elapsed (#398). Once set and exceeded, the same sweep now also evicts finished workbenches oldest-first — never a live session — until back under the cap. Unset or `0` keeps today's unbounded-by-size behavior.

## [0.25.0] - 2026-07-06

Add `moadim enable`/`disable <routine>` CLI commands to toggle a routine's enabled state from the terminal (#820).

Add `GET /routines/{id}/runs/{workbench}/summary`, serving an agent-authored work summary (`summary.md`) for a specific run. Every routine's system prompt now instructs the agent to keep a running work log and write a final summary section to that file before exiting.

### Added


Regression test for `write_tmp`'s ENOSPC/EIO error path (via `/dev/full` on Linux), guarding the fix in #1019 where a full or failing disk during `atomic_write` now propagates the I/O error instead of panicking the whole daemon.

Enforce the 500-line-per-file gate in CI, not just the local pre-push hook (#1029).

### Fixed

Lower the background workbench-cleanup sweep from hourly to every 5 minutes, so a high-frequency routine (e.g. an every-minute schedule, whose effective TTL can be as low as ~60s) no longer piles up dozens of expired, finished workbenches — full repo clones included — between sweeps (#170). The max-runtime watchdog is unaffected; it already runs on its own 30s cadence.

Reject requests with a disallowed `Host` header, and state-changing requests with a cross-origin `Origin` header, closing the DNS-rebinding / browser cross-origin gap against the unauthenticated loopback API (#266). Extend the allowlist for reverse-proxy deployments with `MOADIM_ALLOWED_HOSTS`.

test(routines): cover `next_run_at`'s "no future fire" branch in `model.rs`

`next_run_at`'s doc comment documents three `None` cases — disabled, an
unparseable schedule, and a schedule with no upcoming fire — but only the
first two had tests. `cargo llvm-cov`'s region report showed the third
branch (`cron.iter_after(Local::now()).next()?` returning `None`) was never
exercised. Added a test using a parseable 7-field (`sec min hour dom month
dow year`) schedule pinned to a year that has already passed, so parsing
succeeds but the iterator yields no occurrence.

No behavior change — regression test only.

### Fixed

Run crontab sync (`lock`/`unlock`/`create`/`update`/`delete` on `/routines`) via `tokio::task::spawn_blocking` instead of inline on the async handler. These calls shell out to `crontab`(1); without `spawn_blocking` a slow or hung `crontab` invocation pins a Tokio worker thread indefinitely, and the per-request 30s timeout (`middlewares/timeout.rs`) can't preempt it since the thread is synchronously blocked, not polling a future (#360).

Split service_trigger_tests.rs to satisfy the 500-line pre-push gate; no behavior change (#1018).

Remove a dead duplicate `validate_machines` helper left over after merging with `main`, which already validates machines via `routines::service_validate::validate_machines` (#600).

Write a distinct `killed` sentinel to a watchdog-killed run's `exit_code` file so it never reads back as a misleading clean `0` exit (#453).

## [0.24.0] - 2026-07-06

Fix `atomic_write` panicking the whole daemon on a write/sync I/O error (e.g. disk full) instead of returning it. `File::create`/`open` reserve no disk space, so `write_all`/`sync_all` can still fail after that call succeeds; they now propagate via `?` like every other step in `atomic_write`, instead of `.expect(...)`.

chore(lint): enable `clippy::explicit_into_iter_loop`

Companion to the already-enabled `clippy::explicit_iter_loop`: rejects
`for x in collection.into_iter()` in favor of the equivalent, shorter
`for x in collection`. The workspace was already clean against this lint
(zero violations), so `deny` just locks that in. No behavior change.

Rotate `daemon.log` to a `.log.1` sibling once it exceeds the size cap instead of letting it grow forever — a daemon meant to run unattended for weeks/months must not silently fill the disk. Adds focused unit test coverage for `rotate_daemon_log_if_oversized` (missing file, small file, oversized file, replacing a stale `.1`).

Derive `Eq` alongside `PartialEq` on `Flag`, `RunSummary`, and `FleetRunSummary` (all fields are already `Eq`-safe), and enable `clippy::derive_partial_eq_without_eq` to lock that in for future types.

### Added

- **`list_routine_runs` MCP tool.** Exposes the existing `GET /routines/{id}/runs` run-history
  endpoint over MCP, so an agent can list a routine's past and in-progress runs (workbench id,
  start/finish time, status, exit code) the same way it already can over REST — without needing
  a separate call per run just to fetch `routine_logs`' newest-only log.

test(routines): cover two untested branches in `service_log_tail.rs`

`cargo llvm-cov`'s region report (not the 100%-line gate, which region
coverage doesn't affect) showed two real gaps in the routine log-tail /
ANSI-sanitizing logic used by `svc_logs`/`svc_run_log`:

- `read_log_tail` never had a test for its very first fallible step
  (`std::fs::metadata(path)?`) — a workbench whose `agent.log` was removed
  out from under it (e.g. a racing cleanup sweep) must surface an
  `io::Error`, not panic.
- `strip_ansi_noise`'s OSC-sequence parser only had a test for the
  terminator `ESC \`; the other valid terminator, a bare `ESC` not
  followed by `\`, was never exercised, and it has different behavior
  (the character right after that `ESC` is not consumed, unlike the
  `ESC \` case).

No behavior change — regression tests only.

Enable `clippy::needless_pass_by_value` and fix its three violations: `cli::trigger` now takes `&str` instead of an owned `String`, `cli_query::status_json` takes `Option<&HealthInfo>` instead of an owned `Option<HealthInfo>`, and `LockScope` derives `Copy` (a fieldless enum tag) instead of `global_lock::set_lock` taking it by value under the lint. No behavior change; avoids needless clones/moves at call sites.

feat(routines): surface `is_running` on `GET /routines`/`GET /routines/{id}`

Adds a derived, non-persisted `is_running: bool` field to the routine
response, reporting whether any fire of the routine currently has a live
tmux session. Reuses the existing overlap-guard tmux-prefix probe
(`tmux_session_prefix_alive`, #514) that `svc_trigger` already relies on, so
an operator (or the UI, in a follow-up) can finally tell "is this routine
running right now?" from `GET /routines` instead of shelling in to `tmux ls`.

### Fixed

Split the MCP tool input structs out of `src/routes/mcp.rs` into a new
`src/routes/mcp_types.rs` sibling module. `mcp.rs` had crept to 514 lines,
tripping the pre-push hook's 500-line-per-file gate (`linecheck --max-lines
500`) for every contributor who has `linecheck` installed, as CONTRIBUTING.md
instructs. No behavior change.

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

Fix the built-in "Token Trim" routine's PR step so it clones `~/.config/moadim`'s origin into a disposable `mktemp -d` temp dir and does all branch/commit/push work there, instead of checking out a branch directly inside `~/.config/moadim` — the live checkout the daemon reads routines from. This matches the fix already shipped for the sibling "The 1 Percent" routine (#916); "Token Trim" was never updated to the same pattern, so it still risked leaving the daemon's routines checkout parked on a stale branch mid-run.

## [0.23.0] - 2026-07-05

### Added

- **`moadim enable <routine>` / `moadim disable <routine>` CLI commands** to flip a
  single routine's `enabled` flag from the terminal without opening the web UI or
  hand-crafting a REST `PATCH`. `<routine>` is an id or slug (resolved server-side,
  like the other routine subcommands); the action is idempotent (re-enabling an
  already-enabled routine exits `0`), an unknown routine exits non-zero, and
  `--json` emits a `{"routine","enabled"}` object (#439).

fix(routines): cap agent.log reads to the last 2 MiB

`GET /routines/{id}/logs`, the run-detail log endpoint, and the `logs` MCP
tool all read a run's `agent.log` in full via `read_to_string`. A
long-running or noisy agent can grow that file without bound, so serving it
whole risks an out-of-memory daemon and a multi-hundred-MB HTTP response for
one request. Both now go through a shared `read_log_tail` helper that caps
the read to the most recent 2 MiB, snapped to a UTF-8 character boundary so
a multi-byte character split by the byte-offset seek isn't mangled, and
prefixes a marker noting how many bytes were omitted.

Add tests exercising `prune_project_at`/`prune_locked`'s three previously branch-uncovered `?` error paths in `src/utils/claude_json.rs` (lock-file creation denied, `~/.claude.json` unreadable, and `atomic_write`'s temp-file creation denied). No behavior change — test-only.

feat(cleanup): report freed disk bytes alongside removed count

`POST /routines/cleanup` (and `moadim cleanup`, the `cleanup_workbenches`
MCP tool, and the web UI's CLEANUP NOW button) now report how much disk
space a sweep reclaimed, not just how many workbenches it removed: each
reaped workbench's tree is measured just before deletion and summed into
a new `freed_bytes` field on `CleanupResponse` (additive — existing
`{"removed": N}` consumers are unaffected). `moadim cleanup` prints
`cleanup removed N workbench(es) (freed 12.4 MB)`, and the UI's cleanup
toast mirrors the same humanized size.

feat(cli): add `moadim enable`/`disable <routine>` to flip enabled from the terminal

Toggling a routine's `enabled` state previously required a raw `moadim
routines update <id> --enabled true/false` call or the web UI. `moadim
enable <routine>` / `moadim disable <routine>` now flip it directly (by id
or slug, resolved server-side), printing a human status line or a
`{"routine","enabled"}` object under `--json`. Both are idempotent: setting
an already-enabled routine to enabled again is a no-op success, not an
error.

fix(agents): enable network in the codex default so unattended routines can reach the remote

`codex exec` runs unattended (no approval prompts), but its default
`workspace-write` sandbox blocks outbound network access, so a routine could
not clone the repo or push / open a PR. The built-in codex default now pins
the sandbox to `workspace-write` explicitly and turns network access back on
— the least-privilege setting that still lets the routine reach the remote,
mirroring the baseline the `claude` default already gets.

fix(routines): collision-resistant run id so same-second runs don't clobber

Two runs of the same routine landing in the same wall-clock second (a
double-clicked "Run now", a `trigger` retry, or a manual trigger racing the
scheduled cron fire) derived an identical `$WB`/`$SESS` from `$TS`'s
one-second granularity — the second `tmux new-session` failed with
"duplicate session" and silently no-opped while both clobbered the shared
workbench files. The launch script now mixes the launching shell's PID
(`$$`) into the run id (`$TS_$$`), and fails loudly instead of silently
no-opping if `tmux new-session` still collides. (#411)

fix: don't duplicate a workbench's `runs.log` entry when its removal is retried

The reap sweep persists a finished workbench's outcome to the routine's durable
`runs.log` *before* removing its directory, so `svc_list_runs` still knows
about the run once the workbench is gone. If that `remove_dir_all` then fails
(a permission hiccup, a file still open, a crash), the workbench survives and
gets expired again on the next sweep — which re-persisted the same run,
appending a duplicate `runs.log` entry every sweep the removal kept failing.
The `persist` step now skips workbenches that already have a matching record.

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

chore(lint): enable `clippy::allow_attributes_without_reason`

Every `#[allow(...)]`/`#![allow(...)]` suppression now carries a `reason = "..."`
so a reviewer can tell at a glance why the lint was silenced, and the reason
stays documented as the code evolves. Fixed the handful of bare allows this
newly-`deny`d lint caught (mostly `#![allow(clippy::missing_docs_in_private_items)]`
at the top of test files, plus a `too_many_arguments` and a `zombie_processes`
allow).

chore(lint): enable `clippy::doc_markdown`

Requires backticks around code-like items (type names, paths, identifiers)
in doc comments, so they render as code in generated docs instead of plain
prose. Fixed the one violation this newly-`deny`d lint caught.

chore(lint): enable `clippy::manual_string_new`

Requires `String::new()` over `"".into()`/`"".to_string()` for constructing
an empty `String`. Fixed the two violations this newly-`deny`d lint caught
(both in test fixtures).

fix(service): enable systemd lingering so the daemon survives logout/reboot (#294)

On Linux, `moadim install` starts the daemon under the systemd *user* manager, but without
lingering enabled that manager — and `moadim.service` with it — only runs while the user has an
active login session, so the daemon stopped at logout and never started at boot. `install()` now
runs `loginctl enable-linger` and records a marker file so `uninstall()` disables it symmetrically,
without ever touching lingering the operator enabled themselves for an unrelated reason. Never
fails the install: if `loginctl` is unavailable or errors, a warning with the manual command is
printed instead of aborting.

chore(lint): enable `clippy::todo` and `clippy::unimplemented`

Denies leftover `todo!()`/`unimplemented!()` stubs so they never ship to
production — a stray one would panic the daemon on that code path, same
rationale as the existing `dbg_macro` deny. Zero violations found; no code
changes needed.

fix(routines): extend PATH in run.sh instead of replacing it

The exported `PATH` in a routine's generated launch script now appends the
curated fallback dirs to the login shell's `$PATH` (`export PATH=$PATH:...`)
instead of replacing it outright. Version-manager shim dirs (nvm/pyenv/asdf/volta)
that the profile prepends now survive, so the agent resolves the node/python
the user actually selected; the curated dirs still guarantee `tmux` and the
agent command stay resolvable as a fallback.

chore: fix the three files that had grown past the 700-line pre-push gate

`src/routines/service.rs` (701 lines), `src/routines/cleanup/cleanup_tests.rs` (740 lines),
and `src/service/mod_tests.rs` (816 lines) had all grown past the repo's 700-line pre-push
gate (`.githooks/pre-push`'s `linecheck` step), which currently fails on plain `main` for
anyone who has the git hooks installed per CONTRIBUTING.md.

- Moves `service.rs`'s dozen field-validation helpers (`reject_blank`, `validate_title`,
  `validate_repositories`, `validate_agent`, and friends) into a new
  `src/routines/service_validate.rs`, matching the file's existing convention of
  `#[path = "..."]`-declared sibling modules. No behavior changed; `service.rs` drops to
  457 lines.
- Moves `cleanup_tests.rs`'s tmux/session-probe tests (`tmux_kill_session_is_best_effort_*`,
  `tmux_session_alive_*`, `tmux_session_prefix_alive_*`, and friends) into a new
  `src/routines/cleanup/cleanup_tmux_tests.rs`, matching the existing one-file-per-concern
  convention already used by `cleanup_claude_json_tests.rs` and `cleanup_freed_bytes_tests.rs`.
  `cleanup_tests.rs` drops to 543 lines.
- Moves `mod_tests.rs`'s Linux-only systemd-unit and loginctl/linger tests into a new
  `src/service/mod_linux_tests.rs`, mirroring the macOS/Linux backend split already present
  in `service/macos.rs` and `service/linux.rs`. `mod_tests.rs` drops to 382 lines.

No test bodies changed; `cargo test` and `cargo llvm-cov --fail-under-lines 100` still pass,
and the full pre-push gate (`SKIP_CHANGELOG=1 sh .githooks/pre-push`) now exits 0 on `main`.

fix(routines): derive `agent_registered` from parseability, not file existence

`RoutineResponse.agent_registered` was `true` whenever `<agent>.toml` merely existed on disk, even
if it was malformed. Crontab sync drops such routines via `load_agent_command`, so they never fire
— but the API reported them as healthy. `agent_registered` is now `load_agent_command(...).is_ok()`,
matching what sync actually requires.

fix(routines): fail-fast on a failed agent `setup` step (#287)

A failing `setup` step (e.g. a trust/onboarding pre-seed script) was silently ignored — the
statements are `;`-joined with no `set -e` — so the agent launched anyway, typically hanging on an
interactive prompt with no stdin until the watchdog reaped it roughly an hour later with no
diagnostic. The setup step is now wrapped in a guard that aborts the launch and records the failure
in `agent.log` and on stderr, mirroring the existing `cp prompt.md` guard.

fix: attach pipe-pane atomically with tmux new-session

`pipe-pane` was attached as a separate statement after `new-session -d`, so
any output the agent emitted before the attach (banner, initial plan,
startup crash) was silently dropped from `agent.log`. Both are now chained
in a single `tmux` invocation via `\;`, so the pipe is attached before the
pane can produce output.

fix: rotate daemon.log when it exceeds 10 MiB

`spawn_detached_with()` opened `daemon.log` in pure append mode with no
size cap, so a long-lived install could grow the file unbounded until it
filled the disk. Before opening the log, its size is now checked and
rotated to `daemon.log.1` past 10 MiB (best-effort — a failed rotation
falls through to the existing append-open rather than blocking the spawn).

fix: add global concurrency limit to the HTTP server

The Axum router had no cap on in-flight requests, so a burst of concurrent
requests or a few hung `crontab`/`tmux` calls could exhaust the runtime's
worker/blocking pool and leave even `GET /health` unreachable. A
`tower::limit::GlobalConcurrencyLimitLayer` (a single shared semaphore, cap
64) now sits as the outermost layer, queuing excess requests instead of
piling more work onto the runtime.

### Tests

- Added a `flags_tests` regression guard
  (`list_flags_skips_md_files_that_dont_match_the_flag_shape`) covering two
  `parse_filename` edge cases that were previously untested: a `.md` file with
  no `-` to split a timestamp off of (e.g. `README.md`), and a `.md` file
  whose `-`-delimited suffix isn't a valid timestamp (e.g.
  `bug-notatimestamp.md`). `list_flags` is documented to silently skip
  unparsable filenames rather than error; this locks that contract in against
  a future regression (e.g. someone swapping the `?` for `.unwrap()` in
  `parse_filename`).

docs(cli): document every accepted flag in `moadim --help`

The parser already accepted `-f`/`--foreground` as aliases for `-i`/`--interactive`,
`-d`/`--detach`/`--daemon` as aliases for `-b`/`--background`, and `--version` as a
long form of `-V`, but the help text never mentioned them — a user could only
discover these aliases by reading the source. The help text now documents every
alias the parser accepts, and a new test (`help_text_documents_every_accepted_flag`)
asserts the two can't silently drift apart again.

feat(ical): advertise REFRESH-INTERVAL & X-PUBLISHED-TTL on /routines.ics

The feed is regenerated on every request, but without a refresh hint
subscribers fall back to their own default polling interval (often 12-24h),
making routine schedule edits lag for hours before showing up in a
subscribed calendar. The feed now advertises both the RFC 7986 §5.7
`REFRESH-INTERVAL` property and the widely-honored Microsoft/Google
`X-PUBLISHED-TTL` fallback, both set to one hour.

fix(ical): give the schedule-truncation marker VEVENT a DURATION

The trailing "schedule truncated" marker event appended to the `.ics` feed
when a high-frequency routine hits the 100-event cap carried no `DURATION`
(unlike every regular fire event), so RFC 5545 treats it as a zero-length
instant. Most calendar UIs render a zero-length event as a barely-visible
sliver, defeating the marker's one job of telling subscribers the feed was
capped. It now carries the same `DURATION`/`TRANSP`/`X-MICROSOFT-CDO-BUSYSTATUS`
properties as a regular fire event.

Fixed: deleting a routine while its agent was mid-run left that run executing unsupervised — the workbench's tmux session survived, untracked, until the next TTL sweep reaped the now-orphaned workbench (up to `effective_ttl_secs` later). `svc_delete` now force-kills any still-running session for the deleted routine's slug immediately (issue #333). The workbench directory itself is left in place and reaped normally.

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

fix(server): stop warning on every startup about the openapi.json write

`write_openapi_spec` targets `CARGO_MANIFEST_DIR/apis/openapi.json`, a path
baked in at compile time. For an installed binary (`cargo install`), that
directory is wherever the crate happened to build and generally doesn't
exist on the end user's machine, so every server startup logged a
`could not write openapi spec: ...` warning for a file nobody expects to be
writable there (#319). Skip the write when its parent directory doesn't
exist instead of attempting and warning.

### Fixed

The overlap guard (#514) matched a live tmux session by a plain string prefix (`moadim-<slug>-`), so a routine whose slug is itself a prefix of another routine's slug (e.g. `deploy` vs. `deploy-staging`) could have its own fire silently skipped by an unrelated routine's session — `"moadim-deploy-staging-<rid>".starts_with("moadim-deploy-")` read as "deploy is still running" even when deploy had no session of its own. The match now requires the text after the prefix to have the exact `$RID` shape the launcher emits (`<unix-ts>_<pid>`), so only a genuine fire of the same routine counts.

fix(security): create the daemon's on-disk tree owner-only (#382)

The daemon's on-disk tree is a secret/transcript store (`agent.log` transcripts, `prompt.md`
instructions, token-referencing routine state) but was created at the default umask, landing
directories `0755` and files `0644` — world-readable on a default shared-host umask. Directories
under `~/.config/moadim/` are now created `0700` via `utils::fs_perms::create_private_dir_all`,
files published through `utils::atomic::atomic_write` (routine state, the `prompt.md` sidecar,
`machine.local.toml`) are now created `0600` before the publishing rename, and each routine's
launch script now sets `umask 077` before its first `mkdir` so the workbench directory and
everything written inside it — the copied `prompt.md`, appended `CLAUDE.md`, and tmux-piped
`agent.log` — stay unreadable by other local accounts. Unix-only; non-unix builds are unchanged.
Pre-existing files from older installs are tightened on their next write, not migrated
retroactively.

fix(cleanup): prune stale `~/.claude.json` `projects` entry when reaping workbenches

The built-in `claude` agent's `setup` step seeds a per-workbench entry into
`~/.claude.json`, keyed by the workbench's absolute (always-unique) path, on
every run. Nothing ever pruned it once the workbench was reaped, so the file
grew by one dead entry per `claude` run, forever. Cleanup now removes the
matching `projects[<workbench>]` entry when it reaps a workbench directory,
using the same flock-guarded read -> modify -> atomic-replace pattern the
setup step already uses. (#430)

### Removed

- Removed the vestigial `echo` demo endpoint/tool — the scaffold `POST
  /api/v1/echo` REST route, the `echo` MCP tool, and their `EchoRequest` /
  `EchoResponse` / `EchoInput` types and OpenAPI entries, plus the `moadim
  echo <message>` CLI passthrough. It echoed a message back with a server
  timestamp, served no product purpose, and only widened the REST + MCP +
  OpenAPI + CLI surface; `GET /health` already covers liveness probing. The
  committed `apis/openapi.json` is regenerated without the `/echo` path and
  schemas (#359).

feat(cli): add `moadim restart --interactive/-i` to restart in the foreground

`moadim restart` always backgrounded the fresh instance, so restarting into
an attached, foreground session (to watch startup logs, or under a process
supervisor that expects a foreground child) required a separate `stop` +
`-i`. `restart -i`/`--interactive` now stops any running server, same as
`restart`, but brings the fresh instance up in the foreground instead of
detaching it — mirroring `moadim -i`.

feat(cli): add `--json`/`--quiet` to `moadim restart`

`moadim restart` only printed human-readable status lines, so scripts had
no clean way to consume its result. `--json` now emits a single
machine-readable object (`{"old":N|null,"new":N,"address":…}`, matching the
shape every other `--json` lifecycle command surfaces), and `--quiet`
prints just the `restarted: pid <old> -> <new>` rotation line, suppressing
the UI/stop/logs hint block, for script-friendly output without the
overhead of JSON parsing.

feat(routines): expose `next_run_at` on the routine API response

`GET /routines` (and single-routine reads) already surfaced `schedule`,
`schedule_description`, and `timezone`, but never the computed next fire
time — you had to mentally evaluate the cron expression, open the
CALENDAR view, or subscribe to the `.ics` feed to find out when a routine
runs next. `RoutineResponse` now includes `next_run_at` (Unix epoch
seconds, host-local-timezone crontab semantics), reusing the same
`croner` evaluation the `.ics` feed and TTL sweep already perform. It is
`null` when the routine is disabled, the daemon is globally locked, or
`schedule` is unparseable or has no upcoming fire (e.g. `@reboot`).
Closes #369.

### Fixed

A routine had no overlap guard: nothing stopped a new fire from launching while a previous fire of the *same* routine was still running. A routine whose agent run outlived its schedule interval (e.g. `* * * * *` with a slow agent) would pile up concurrent tmux sessions all acting on the same target — duplicate PRs/issues, racing git pushes. Both the manual and scheduled trigger paths now check for a live tmux session under the routine's `moadim-<slug>-` prefix before launching, and skip the fire (with a warning) if one is still active.

feat(routines): per-routine power-saving mode, orthogonal to enabled

Adds `power_saving: bool` alongside the existing user-owned `enabled` toggle:
both must hold (`enabled && !power_saving`) for a manual or scheduled trigger
to launch. `power_saving` is system/policy-owned, never touched by
create/update, and persisted in the gitignored `state.local.toml` sidecar like
`snoozed_until`/`skip_runs` rather than the tracked `routine.toml`. Set/cleared
via the new `set_power_saving` MCP tool. The web UI's health badge and
"Run now" tooltip distinguish `POWER SAVING` from `DISABLED`.

fix(routines): dedupe `tags` on create/update, mirroring `machines`

`validate_tags` trimmed and rejected blank entries but never collapsed
duplicates, unlike its `validate_machines` sibling. A duplicate (or
whitespace-padded repeat) tag such as `["nightly", "nightly"]` or
`["nightly", " nightly "]` persisted verbatim, rendering as a doubled chip
in the routine row and an inflated tag list for a label that names one
concept once. `validate_tags` now dedupes on the trimmed value, keeping the
first occurrence, matching `validate_machines`'s existing behavior.

feat(routines): persist run history past workbench TTL reaping

`svc_list_runs`/`svc_list_all_runs` (and the HISTORY page / RECENT RUNS
panel that read them) used to show only runs whose workbench directory
was still on disk — once TTL-reaped, a run's outcome vanished. The reaper
now appends a compact record (workbench, timestamps, status, exit code)
to each routine's `runs.log` right before removing its workbench, so a
routine's run history survives past its configured retention window (the
`agent.log` body itself is still discarded, since retaining full logs
forever isn't the retention knob's job).

fix(service): restart only on failure so a clean stop stays stopped

The systemd unit and launchd agent restarted the daemon on *any* exit
(`Restart=always` / unconditional `KeepAlive`), so a clean shutdown via
`moadim stop`, the UI STOP button, or `POST /shutdown` was resurrected by
the supervisor ~5s later. Restart is now failure-only
(`Restart=on-failure` / `KeepAlive = { SuccessfulExit = false }`): a crash
still auto-restarts, but a clean stop stays stopped.

fix(server): bound graceful-shutdown drain so `moadim stop` can't hang forever

`axum`'s graceful shutdown waits for every in-flight connection to close
before returning, so a long-lived stream (an `/mcp` SSE subscription, a slow
client) could keep that future pending indefinitely, hanging `moadim
stop`/`POST /shutdown` forever (#342). The server now caps the post-shutdown
drain to a bounded grace window (10s by default, overridable via
`MOADIM_SHUTDOWN_GRACE_MS` for tests) and forces a clean exit once it
elapses, logging a warning if connections were still open.

chore(cli): split the bind-override tests out of `cli_tests.rs`

`src/cli_tests.rs` had grown to 705 lines, past the repo's 700-line pre-push
gate (`.githooks/pre-push`'s `linecheck` step), which currently fails on
plain `main` for anyone who has the git hooks installed per CONTRIBUTING.md.

Moves the 7 `BIND_ADDR_ENV`-override tests (`bind_addr_uses_default_when_unset`,
`bind_addr_honors_override`, `status_json_address_reflects_bind_override`, and
friends) into a new `src/cli_bind_override_tests.rs`, with its own `EnvGuard`
copy, matching the existing one-helper-copy-per-test-file convention already
used by `cli_json_tests.rs` and `cli_spawn_tests.rs`. No test bodies changed;
`cli_tests.rs` drops to 617 lines. `cargo test` still passes the same 101
`cli::*` tests.

chore(routines): split `command_tests.rs`'s binary-resolution tests into a sibling file

`src/routines/command_tests.rs` had grown to 713 lines, past the repo's 700-line pre-push
gate (`.githooks/pre-push`'s `linecheck` step), which currently fails on plain `main` for
anyone who has the git hooks installed per CONTRIBUTING.md.

Moves the 18 `tmux`/agent-binary-resolution tests (`tmux_available_*`,
`agent_command_available_*`, `resolve_tmux_bin_*`, `bin_dir_returns_none_when_path_unset`,
`tmux_fallback_dirs_are_anchored_under_home`) into a new
`src/routines/command_bin_resolution_tests.rs`, matching the existing one-helper-copy-per-
test-file convention already used by `command_run_id_tests.rs` and friends. No test bodies
changed; `command_tests.rs` drops to 462 lines. `cargo test` still passes with the same 886
tests, and `cargo llvm-cov` shows `routines/command.rs` unchanged at 100% line coverage.

fix(routines): strip ANSI escapes and `\r`-redraw noise from served logs

`tmux pipe-pane -o` captures a routine's pane output verbatim, so `GET
/routines/{id}/logs`, the run-detail log endpoint, and the `logs` MCP tool
all served raw terminal escape sequences (color codes, cursor movement,
screen clears) and every redraw frame of a spinner or progress bar as its
own line, instead of the final state a real terminal would display (#278).
`read_log_tail` now strips ANSI/VT escape sequences and collapses
`\r`-based redraw overwrites down to the last write per line before
returning content, so served logs read as the logical lines an operator
would actually see in a terminal.

fix(routines): make svc_list deterministic for tied sort keys

Routines come off a `HashMap`, whose iteration order is unspecified, so a
listing sorted by a field with duplicate values (e.g. several routines
created in the same second) previously rendered in an arbitrary, run-to-run
order. `svc_list` now breaks ties on the stable routine id, and reverses the
whole comparison (not just the sorted vector) for descending order so the
tiebreak direction stays consistent.

fix(routines): tombstone a deleted built-in default so it stays deleted

Deleting a built-in default routine now records its slug in a tombstone
file (`removed_default_routines_path`), so the next startup's
`ensure_default_routines` no longer resurrects it enabled. Re-creating a
routine under a tombstoned default's title clears the tombstone, since
that is a deliberate "bring it back" signal.

Add unit tests for the routine HISTORY page's `fmt_run_duration` formatter
and its run-status badge class/label helpers — they shipped untested,
including the `finished_at < started_at` underflow-guard branch.

feat(ui): add a fleet-wide RECENT RUNS panel to the overview page

`GET /routines/runs?limit=N` returns the most recent runs across every
routine (newest first, one workbench-directory scan) instead of one
`/routines/{id}/runs` request per routine. The overview page's new RECENT
RUNS table uses it to show what just ran fleet-wide, complementing the
existing UPCOMING RUNS panel (future fires) with the equivalent view of
the past.

fix(ui): deep-link RECENT RUNS entries straight to that routine's HISTORY page

Clicking a routine name in the overview page's RECENT RUNS panel used to
land on the plain routine list. It now carries a `?history=<id>` query
that the routines page reads on mount and opens that routine's HISTORY
page directly — one click from "what just ran, fleet-wide" to the full
per-run detail.

feat(ui): add DURATION column to the Overview "RECENT RUNS" table

The fleet-wide recent-runs table on the Overview page previously showed
ROUTINE / STARTED / STATUS / EXIT CODE. It now also shows DURATION (wall-clock
elapsed between started_at and finished_at), matching the same column that
already exists on each routine's own HISTORY page.

Add a **Model** field to the routine create/edit form and Routines table row. The backend already persisted an optional `model` override per routine and passed it to the agent invocation as `--model` (`src/routines/model.rs`, `src/routines/command.rs`), but the UI never exposed it — this wires up the missing free-text input, save/clear round-trip, and row display (#742).

feat(ui): make UNREGISTERED AGENT stat tile a clickable filter

The "UNREGISTERED AGENT" tile on the routines stats bar was a read-only
display div. It is now a clickable filter button (like DORMANT, FLAGS,
SNOOZED) that filters the table to show only routines whose agent is not
registered. The tile turns amber when any unregistered-agent routines exist.
A new `AgentUnregistered` variant is added to `RoutineStatusFacet` so
the filter state persists in the URL via the existing `status=` query param.

feat(ui): add MACHINES column to the routines table

The routines table now has a MACHINES column showing how many machines
each routine is assigned to. When a routine has no machines assigned
(dormant) the cell shows an amber "—" instead of a number, so operators
can spot un-targeted routines without filtering. Hovering the count
shows the full list of machine names.

feat(ui): add TAG filter to the routines filter bar and include tags in search

Two improvements to tag-based visibility:
- Tags are now included in the free-text search haystack, so typing a tag name
  into the search box narrows the list to routines carrying that tag.
- When any routines have tags, a TAG drop-down appears in the filter bar,
  allowing operators to filter the table to a single tag without using the
  search box. The drop-down is hidden when no routines are tagged.

feat(ui): add a run-history page for routines

Routines now record the exit code of every run (written to the workbench
by the launch command once the agent process exits) and expose it via
`GET /routines/{id}/runs` and `GET /routines/{id}/runs/{workbench}/log`.
A new HISTORY button on each routine row opens a page listing every kept
run — start time, status (RUNNING/SUCCESS/FAILED/UNKNOWN), duration, and
exit code — with a per-run log viewer, instead of the LOGS page's
newest-run-only view.

feat(ui): add a Settings page for the persistent agent prompt

`~/.config/moadim/user_prompt.md` — the prompt text appended to every
routine's agent instructions file — was previously editable only by hand
on disk. `GET`/`PUT /config/user-prompt` now expose it over the REST API,
and a new SETTINGS page (nav tab + command palette entry) lets it be
viewed and edited from the UI. Machine identity and the global schedule
lock keep their existing header/banner controls; this page covers the one
setting that had no UI surface at all.

feat(ui): expose Snoozed, Flagged, and Agent-unregistered options in the STATUS filter dropdown

Three status facets (`Snoozed`, `HasFlags`, `AgentUnregistered`) were fully
implemented in the filter logic but had no corresponding option in the STATUS
drop-down, making them invisible to users. Operators can now select:

- **Snoozed** — routines whose scheduled fires are currently suppressed
- **Flagged** — routines with one or more open flags needing review
- **Agent unregistered** — routines whose agent config is missing

fix(cli): unknown command exits 2 to stderr, not help to stdout

An unrecognized first argument (e.g. `moadim staus`) is no longer treated
as a successful `help` invocation. It now prints `unknown command: <arg>`
plus a hint to stderr and exits `2`, distinct from an explicit
`help`/`-h`/`--help` request (stdout, exit `0`).

fix(routines): validate agent-config args placeholders

Creating or updating a routine now validates the referenced agent's `args`
against two silent fire-time failures (#322): a typo'd placeholder token
(e.g. `{prompt_fil}`) that would reach the agent as a literal, dead argument,
and a config with no `{prompt}`/`{prompt_file}` placeholder at all, which
would launch the agent with no task. Both are rejected with `400 Bad
Request` at edit time instead of silently burning a run at fire time.

## [0.22.1] - 2026-07-05

Enable `clippy::needless_pass_by_ref_mut` in `[lints.clippy]`. The codebase was already clean against it (no violations), so this is a lint-only change that locks in the invariant that every `&mut` parameter is actually mutated through.

fix: skip dirs with no routine.toml in the prompt-subfolder migration, so an orphaned routines/ dir no longer gets an empty prompts/prompt.pure.md resurrected on every startup

chore: lower linecheck gate from 1500 to 1000 lines

chore: lower linecheck gate from 2000 to 1500 lines

chore: lower linecheck gate from 2500 to 2000 lines

chore: lower linecheck gate from 1000 to 700 lines

### Fixed

- `moadim stop` now sticks under a service install. The systemd unit and launchd
  agent restarted on *any* exit (`Restart=always` / unconditional `KeepAlive`),
  so a clean shutdown was resurrected by the supervisor ~5s later and `stop`
  reported a false success. Restart is now failure-only (`Restart=on-failure` /
  `KeepAlive = { SuccessfulExit = false }`): a crash is still auto-restarted, but
  a clean stop stays stopped. (#444)
- Routine listings (`GET /routines`) are now deterministic when several routines
  share the same sort key. The list is built from a `HashMap`, whose iteration
  order is unspecified, so equal-key routines previously came back in an
  arbitrary, run-to-run order. Ties are now broken on the stable routine id, and
  descending order reverses the comparison rather than the sorted vector so the
  tiebreak stays consistent.
- Routine iCal feed events are now `TRANSP:TRANSPARENT` instead of the default
- The built-in `codex` agent default now enables sandbox network access
  (`codex exec -s workspace-write -c sandbox_workspace_write.network_access=true`).
  `codex exec`'s default workspace-write sandbox blocks the network, so a
  codex-backed routine could not clone the remote repo or push / open a PR — it
  would silently no-op while still showing a healthy routine. This brings codex
  to parity with the `claude` default's unattended-access baseline; the setting
  is overridable in `~/.config/moadim/agents/codex.toml`. (#449)
- **The routine LOGS view search/highlight could panic the UI on ordinary Unicode log content.** `highlight()` found matches by lowercasing the whole line and then reapplying *those* byte offsets to the original (un-lowercased) string. Case folding isn't always byte-length-preserving (`ẞ`, U+1E9E, 3 bytes, lowercases to `ß`, U+00DF, 2 bytes) or even char-count-preserving (Turkish `İ` expands to two chars), so a matching search query after such a character could compute an offset that lands mid-character and panics on the slice (crashing the Yew render for that log line). The matching logic now projects each original char to exactly one lowercase char and tracks byte spans via `char_indices()`, so every slice boundary is guaranteed valid regardless of the log content's script.

fix(ui): derive the Overview page's per-source `snoozed` flag from the same `now` already threaded through its KPI/attention/upcoming-run math instead of sampling `js_sys::Date::now()` inline, so `is_snoozed`/`from_routine`/`sources_of` stay deterministic and host-testable (this was silently broken: `cargo test --workspace` panicked with "cannot call wasm-bindgen imported functions on non-wasm targets" in 4 `overview_tests`, invisible in CI because `test.yml` only runs bare `cargo test`, which skips the `ui` workspace member).

feat(ui): show open flag count in NEEDS ATTENTION detail column

The NEEDS ATTENTION panel now shows "N open flag(s) — needs review"
instead of a generic detail string for HasOpenFlags rows, so operators
can see the severity at a glance without navigating into the routine.
AttentionItem now carries flag_count; a new test verifies it is
correctly propagated from SchedSource.

feat(ui): surface routines with open flags in the NEEDS ATTENTION panel

The overview's NEEDS ATTENTION panel caught config problems (DORMANT,
DEAD SCHEDULE, AGENT MISSING) but was blind to runtime issues: routines
whose agents raised flags during a run never appeared there. Operators
had to discover flagged routines by scanning the routines table.

Adds HasOpenFlags as an attention reason (rank 3, lowest priority so
config faults still surface first). An enabled routine with flag_count > 0
now appears in the panel with an "OPEN FLAGS — agent raised flags during
a run — needs review" badge.

Three new tests: open flags surfaces when otherwise healthy, config
faults outrank flags, disabled routines with flags remain hidden.

feat(ui): mute snoozed routines in the month calendar view

Calendar chips for currently-snoozed routines now render at 45% opacity
with an amber left border, matching the treatment added to the day
timeline, so operators can see at a glance which future fire times belong
to suppressed routines.

feat(ui): show snoozed and flag indicators on day-timeline chips

Day-view timeline chips now carry two additional signals:

- **Snoozed routines** render at 45% opacity with an amber left-border
  instead of the standard accent border, so operators can distinguish
  suppressed fire times from active ones at a glance.
- **Flagged routines** show a red `⚑N` badge on the chip so pending
  flags are visible without leaving the timeline view.

feat(ui): surface dependency health warnings and build info in the header

Show "⚠ NO TMUX" (red, pulsing) and "⚠ NO PYTHON3" (amber) warning badges in
the header when the daemon reports a missing runtime dependency. Extends the
`Health` struct to include `dependencies` and `git_sha` from the existing
`/api/v1/health` response, and displays the git SHA as a tooltip on the version
label.

feat(ui): add DORMANT tile to routines stats bar

The routines stats bar now shows a DORMANT tile — the count of enabled
routines assigned to no machine (they are enabled but will never fire).
The tile turns amber when any dormant routines exist and acts as a
clickable filter, narrowing the table to dormant routines.

refactor(ui): extract inline styles from index.html to styles.css

Moves the app's CSS out of a 1600+ line inline `<style>` block in
`ui/index.html` into `ui/styles.css`, linked via trunk's
`data-trunk rel="css"` asset pipeline. The self-hosted font-face
data-URI stays inline.

feat(ui): show flag age in the flags panel

Each flag row now shows a relative timestamp ("3h ago", "2d ago") next
to the scope badge, so operators can see at a glance how long a flag
has been open without cross-referencing the file metadata.

feat(ui): show open flag count header in flags panel

The flags panel now shows "N open flag(s)" above the flag list so
operators immediately see the total count without scrolling to the end.

feat(ui): make FLAGS tile in stats bar a clickable filter

The FLAGS tile in the routines stats bar was informational-only. It is
now a clickable filter button (like SNOOZED, DUE SOON, etc.) that narrows
the table to routines with one or more open flags. Clicking it again
clears the filter. The tile border and value turn red when any flags are
present.

Adds `RoutineStatusFacet::HasFlags` with codec roundtrip and one new host
test.

feat(ui): add FLAGS KPI tile to overview dashboard

Surface the total count of open flags across all routines as a FLAGS
tile in the overview stat row. Red when non-zero, green when clear —
gives operators an at-a-glance signal without navigating into individual
routines. Adds `flag_count` to `SchedSource` and `flags` to `Kpis`.

feat(ui): add Health option to routines group-by selector

The routines GROUP BY selector now includes "Health" as an option.
Choosing it partitions the routine list by the derived health badge
(HEALTHY, SNOOZED, DORMANT, DEAD SCHEDULE, AGENT MISSING, DISABLED),
making it easy to scan which routines share the same health state.

feat(ui): add RefreshControl to heatmap page

The heatmap page previously used a hard-coded 30 s background refresh
with no user-visible indicator of when data was last loaded. It now
shows the same RefreshControl as the Overview and Routines pages
(Off / 5s / 15s / 30s / 60s dropdown + "updated N ago" freshness cue),
sharing the same localStorage key so the chosen cadence is consistent
across all pages.

feat(ui): add SOURCES KPI tile to the schedule heatmap

The heatmap stats bar now shows a SOURCES tile — the number of enabled
routines that contributed at least one fire to the 7-day grid. This lets
operators quickly distinguish a high-density grid (many routines, many
fires) from a high-frequency grid (few routines, very frequent fires).

chore(ui): extend the 700-line linecheck gate to the `ui` crate

feat(ui): show freshness cue in logs and flags page headers

The LOGS and FLAGS sub-pages now show "updated just now" / "updated Nm ago"
in the page header after each load or manual refresh, matching the pattern
already used on the Overview, Routines, and Heatmap pages.

fix(ui): show "snoozed" in NEXT RUN cell instead of suppressed fire time

When a routine is snoozed its scheduled fires are suppressed, but the
NEXT RUN column still showed the upcoming time as if the run would happen.
Now shows "snoozed" (muted, consistent with "paused" for disabled
routines) so the table accurately reflects what will execute.

Extracts `is_routine_snoozed` as a shared helper used by both
`routine_health` and `next_routine_run_cell`, with four dedicated tests.

feat(ui): add DORMANT KPI tile to the overview page

The overview KPI row now includes a DORMANT tile — the count of enabled
routines assigned to no machine (they are enabled but will never fire).
The tile turns amber when any dormant routines exist, matching the DORMANT
tile already present on the Routines page stats bar.

feat(ui): show global lock banner on the overview page

When routines are globally locked the OVERVIEW page now shows the same
warning banner as the Routines tab. Previously users on the overview had
no indication that scheduling and manual triggers were paused — they had
to navigate to another tab to discover the lock. The banner shows which
sentinels are active (SHARED .lock / LOCAL .local.lock) and is fetched
alongside the routine list on every refresh cycle.

feat(ui): add RefreshControl to overview page

The overview page previously used a fixed 30 s background refresh with
no user-visible indicator of when data was last loaded. It now shows the
same RefreshControl as the Routines page (Off / 5s / 15s / 30s / 60s
dropdown + "updated N ago" freshness cue), sharing the same
localStorage key so the chosen cadence is consistent across pages.

feat(ui): add UNLOCK ALL button to the overview page lock banner

The overview page's lock banner previously showed "ROUTINES GLOBALLY
LOCKED" as a read-only notice. It now renders the same `GlobalLockBanner`
component used on the Routines page, which includes an UNLOCK ALL button.
Operators no longer need to navigate to the Routines tab to clear a lock.

feat(ui): show routine health status tags in command palette subtitles

Routine entries in the ⌘K command palette previously showed only the
schedule description. They now suffix status tags so operators can see
health issues without leaving the palette:

- "DISABLED" — routine is turned off
- "SNOOZED" — skip_runs counter is active
- "AGENT MISSING" — agent not registered
- "FLAGS" — one or more open flags (appended alongside any other tag)

Six new host tests cover the combinations.

feat(ui): include routine tags in command palette search keywords

Routine tags are now indexed as search keywords in the command palette
(⌘K), so typing a tag name (e.g. "security", "weekly") surfaces all
matching routines without needing to know their exact titles.

feat(ui): show repository names on hover in routines REPOS column

The REPOS count cell previously showed only a number with no way to see
which repositories were linked without opening the edit form. Hovering
now shows a newline-separated list of repository names as a native
browser tooltip.

feat(ui): show routine goal as subtitle in routines table TITLE column

Routines with a goal set now show the first line of the goal text as a
muted subtitle beneath the routine name in the TITLE column. Hovering
reveals the full goal text. This surfaces the "why" behind the routine
directly in the table without requiring the operator to open the edit form.

feat(ui): make UNREGISTERED AGENT stat tile a clickable filter

The "UNREGISTERED AGENT" tile on the routines stats bar was a read-only
display div. It is now a clickable filter button (like DORMANT, FLAGS,
SNOOZED) that filters the table to show only routines whose agent is not
registered. The tile turns amber when any unregistered-agent routines exist.
A new `AgentUnregistered` variant is added to `RoutineStatusFacet` so
the filter state persists in the URL via the existing `status=` query param.

feat(ui): add MACHINES column to the routines table

The routines table now has a MACHINES column showing how many machines
each routine is assigned to. When a routine has no machines assigned
(dormant) the cell shows an amber "—" instead of a number, so operators
can spot un-targeted routines without filtering. Hovering the count
shows the full list of machine names.

feat(ui): add SNOOZED and FLAGS tiles to routines page stats bar

The Routines page stats bar previously only showed TOTAL, ENABLED,
DISABLED, DUE SOON, and UNREGISTERED AGENT. It now also shows:

- **SNOOZED** — count of routines with suppressed fires (clickable
  filter like DUE SOON; amber when non-zero)
- **FLAGS** — total open flags across all routines (red when non-zero)
- **DUE SOON** — now correctly excludes snoozed routines (same fix as
  the overview page in #945)

Adds `Snoozed` to `RoutineStatusFacet` with roundtrip codec support and
a filter-matching test.

feat(ui): show snooze-until detail in the NEXT RUN cell

Snoozed routines previously showed only "snoozed" in the NEXT RUN
column. The cell now includes a secondary line with context:

- "Nm left" / "Nh left" / "Nd left" — when a `snoozed_until` deadline
  is set, showing how long until the routine resumes automatically.
- "N run(s) skipped" — when a `skip_runs` counter is active.

Seven new host tests cover all the formatting branches.

fix(ui): exclude snoozed routines from DUE SOON count and UPCOMING RUNS table

Snoozed routines appeared in the overview's DUE SOON KPI and UPCOMING
RUNS table as if they would fire, even though their scheduled fires are
suppressed. Fixes both to only include enabled, non-snoozed sources so
the dashboard reflects what will actually run.

feat(ui): add SNOOZED KPI tile to overview dashboard

Surface the count of enabled routines whose scheduled fires are
currently suppressed (snoozed or skip-runs active) as a SNOOZED tile
in the overview stat row. Amber when non-zero, green when clear —
makes it immediately visible when routines are intentionally silenced.

feat(ui): show open-flag badge on upcoming-runs rows

The UPCOMING RUNS table now shows a small "⚑ N" badge next to the name
of any routine that has open flags, so operators can see at a glance
which about-to-fire routines still need flag review without navigating
to the NEEDS ATTENTION panel. Two new tests verify the flag count is
correctly propagated from SchedSource to UpcomingRun.

feat(ui): show raw cron in upcoming runs when no human description exists

The SCHEDULE column in the upcoming runs table previously showed "—" for
routines whose daemon had not yet computed a human-readable description.
It now falls back to the raw cron expression (e.g. `*/15 * * * *`) so
operators always see something actionable.

## [0.22.0] - 2026-07-03

### Changed

- **Pre-push linecheck gate lowered from 3000 → 2500 lines per `.rs` file.** `service_tests.rs` was split again to comply — tags, machines, and model tests now live in `service_model_tests.rs`.

### Changed

- **Pre-push hook linecheck gate lowered from 3000 → 2500 lines per `.rs` file.** `service_tests.rs` (2677 lines) was split into `service_tests.rs` and `service_model_tests.rs` to satisfy the new ceiling.

### Changed

- **Pre-push hook now rejects `.rs` files exceeding 3000 lines.** Step 6 of `.githooks/pre-push` runs `linecheck --max-lines 3000` over all `src/**/*.rs` files. `cargo install linecheck` is required. `service_tests.rs` (3068 lines) was split into `service_tests.rs` and `service_flag_tests.rs` to satisfy the new gate.

### Changed

- **Scheduled and manual trigger history is now recorded in append-only `.log` files.** `scheduled.local.toml` (overwritten on each cron fire) is replaced by `scheduled.log`; the manual-trigger timestamp previously stored in `state.local.toml` moves to `manual.log`. Each file records one Unix timestamp per execution, giving a full run history instead of only the most recent timestamp. A startup migration seeds the log files from any legacy TOML sidecars found on disk and removes the old files, so existing installs upgrade transparently. The `.log` suffix matches the existing `*.log` gitignore pattern seeded into each routine directory.

## [0.21.0] - 2026-07-03

### Added

- Routine create/update now validates the referenced agent config's `args`
  placeholders: a typo'd token (e.g. `{prompt_fil}`) or `args` that contain no
  `{prompt}`/`{prompt_file}` at all are rejected with a `400 Bad Request` at edit
  time, naming the offending token. Previously such a config passed the
  agent-registered check and only failed at fire time — launching the agent with
  a garbage or empty task and burning a full run until the watchdog reaped it
  (#322).
- The routines iCalendar feed (`/routines.ics`) now advertises a one-hour poll
  hint via the RFC 7986 `REFRESH-INTERVAL;VALUE=DURATION:PT1H` property plus the
  widely-honored `X-PUBLISHED-TTL:PT1H` fallback. The feed is regenerated per
  request, so without a hint subscribers fall back to their client's slow default
  (often 12–24h) and routine changes lag for hours; the hint asks them to poll
  hourly instead.
- **Per-routine power-saving mode.** A routine can now be paused for power
  saving independently of its `enabled` toggle — `enabled` stays user-owned
  intent, `power_saving` is a separate, system/policy-owned throttle that both
  must clear for a firing to launch (`enabled && !power_saving`). Set/cleared
  via the new `set_power_saving` MCP tool (`svc_set_power_saving`); persisted
  in the gitignored `state.local.toml` sidecar like `snoozed_until`/`skip_runs`,
  never in the tracked `routine.toml`, and never touched by create/update. Both
  `trigger_routine` and the routine's cron schedule now refuse to launch while
  it (or `enabled: false`) is active, with a distinct message naming which one.
  The web UI's health badge and "Run now" tooltip distinguish `POWER SAVING`
  from `DISABLED`. (#95)

- **Optional `goal` for routines.** A routine can now carry a very short (at most
  5 lines) statement of its goal — the "why" behind the prompt. It is optional
  (default unset), persisted in the tracked `routine.toml`, and rendered into the
  agent's `prompt.md` as a `## Goal` preamble ahead of the task. Settable across
  every surface: REST (`goal` on the create/update bodies), MCP
  (`create_routine`/`update_routine`), the CLI (`--goal` on
  `routines create|replace|update`), and the web UI (a field in the routine
  form). The value is trimmed; a goal longer than 5 lines is rejected with
  `400 Bad Request`, and sending an empty string on update clears it. The three
  built-in default routines now ship with a goal. (#827)

### Fixed

- **"The 1 Percent" routine no longer mutates the live `~/.config/moadim` checkout.** Its PR step now clones the routines repo to a disposable temp directory and does all branch/commit/push work there, instead of running `checkout -b` / `commit` / `push` directly against the daemon's own routines checkout. This avoids leaving that checkout parked on a stale branch after merge and avoids racing the daemon's own reads of the routines folder.

### Added

- **`agent_command_available` on routine responses.** `RoutineResponse` now
  reports whether the routine's agent `command` (e.g. `claude`, `codex`)
  actually resolves on the daemon's `PATH`, distinct from the existing
  `agent_registered` (which only checks that `<agent>.toml` exists). A
  routine with a present, well-formed agent config but an uninstalled binary
  previously looked identically healthy to one that could actually run.

### Added

- **A 30s per-request deadline on the REST API (`/api/v1/**`).** Previously the router had no request timeout at all: a wedged handler (e.g. blocking `crontab`/`tmux`/filesystem I/O with no `spawn_blocking`, #360) could hold its connection and a Tokio worker open forever with no upper bound and no error response. `POST`/`GET`/etc. requests to `/api/v1/**` that exceed the deadline now abort with `408 Request Timeout` instead of hanging indefinitely. The long-lived `/mcp` SSE stream is deliberately left outside this layer so legitimate streaming connections are unaffected (#402).

### Changed

- **Bumped `rmcp` from 1.7.0 to 2.0.0.** The MCP SDK's `Content`/`RawContent`
  wrapper was replaced by the flat `ContentBlock` enum
  (`Text`/`Image`/`Audio`/`Resource`/`ResourceLink`); the tool-result
  constructors and test assertions were updated to match the new API. No
  behavioral change for MCP clients.

### Changed

- Bumped `yew-router` from `0.18` to `0.20` and `yew` from `0.21` to `0.23`
  (a required companion bump — `yew-router` 0.20 depends on `yew` 0.23) in
  the `ui` crate. No source changes needed beyond the version bump; the
  bundled Yew/WASM SPA builds and behaves the same.

### Fixed

- **Routine list no longer shows stale data after a page reload.** The server now sends `Cache-Control: no-store` on all API responses that don't already carry a cache directive, preventing browsers from heuristically caching `GET /routines` responses and serving stale JSON on reload.

### Added

- Group-by dimension selector for the Routines table (agent, machine, or status), mirroring the existing CronJobs page feature.

### Added

- **`--model` on the `moadim routines` CLI.** `create`, `update`, and
  `replace` gain a `--model <id>` flag, threaded into the same JSON body the
  REST route already accepts. The `model` field itself landed data/API-only
  in #742 with a note that other surfaces were a follow-up; this closes the
  gap for the terminal (the web UI form field remains a separate follow-up).
  On `update`, `--model ""` clears the override back to the agent's own
  default, matching the existing REST/MCP semantics.

Enable `clippy::unnecessary_debug_formatting` and fix the three flagged `log::warn!` call sites (`routine_storage::migrate_prompt_files_from_dir`, `routines::agents::ensure_default_agents_in`) that Debug-formatted (`{path:?}`) a `Path`/`PathBuf` in a user-facing log line instead of using `.display()`, matching every other path already printed this way in the codebase.

Enable `clippy::use_self` (issue #724) and replace the flagged `Type::Variant` spellings with `Self::Variant` inside their own `impl`/`match` blocks across `src/error.rs`, `src/machine/mod.rs`, `src/routines/agents/mod.rs`, `src/routines/flags.rs`, `src/sync/mod.rs`, and their test modules — purely mechanical, no behavior change.

### Added

- **`README.md` seeded into the config directory and its generated
  subdirectories.** On every start, the daemon now writes a `README.md` into
  `{config_dir}` (default `~/.config/moadim/`), `{config_dir}/routines/`, and
  `{config_dir}/agents/` if one doesn't already exist there, explaining each
  folder's layout — the top-level file covers the daemon-managed files
  (`.gitignore`, `machine.local.toml`, `moadim.pid`, `daemon.log`), the
  `routines/` one covers the per-routine directory structure
  (`routine.toml`, `prompts/`, `flags/`, the `.local.` sidecars), and the
  `agents/` one covers the agent registry format. So anyone who opens or
  git-tracks the config folder directly has an orientation doc without
  needing to consult the project README. Never overwrites an existing
  `README.md`, so user edits are preserved.

Serialize the crontab read-modify-write across concurrent syncs so overlapping `crontab -l` → edit → `crontab -` round trips can no longer interleave and clobber each other's writes.

### Fixed

- **A crontab-sync write failure panicked the daemon instead of degrading gracefully.** `write_crontab` piped the routine schedule into `crontab -` and `.expect()`'d both the stdin write and the child's exit status. If the external `crontab` process ever closed its end of the pipe early (e.g. it rejects malformed input mid-stream), the write failed with a broken-pipe error that panicked the request thread — even though every caller of crontab sync already treats a `SyncError` as warn-and-continue, not fatal. Both failure paths now propagate a `SyncError::Io` instead of panicking, and the child is always reaped via `wait()` even when the write fails.

Correct the README and `commands.rs` module doc, which claimed the CLI exposes "every" routine action the REST API and MCP tools do — routine flags (`create_flag`/`list_flags`/`resolve_flag`) and the global routine lock (`get_lock_status`/`lock_routines`/`unlock_routines`) have no `moadim` subcommand and are REST/MCP-only.

Clarify in doc comments and the OpenAPI spec that `PUT /routines/{id}` is a partial-merge alias for `PATCH`, not a full RFC 7231 replace (#872).

### Added

- **Escape dismisses open UI modals/dialogs.** The shutdown-confirm and
  rename-machine dialogs and the routine edit/delete-confirm modals now all
  close on `Esc`, matching the command palette's existing behavior.

### Fixed

- **`cargo doc` no longer fails on `main`.** The doc comment on `sh_bin()` in
  `src/routines/service.rs` used an intra-doc link
  (`` [`crate::sync::crontab_bin`] ``) to a private, unexported function,
  which rustdoc can never resolve even with `--document-private-items`. This
  tripped `#![deny(warnings)]` and broke the `cargo doc` CI check (and any
  local `cargo doc` / `cargo install moadim` doc build) on every PR
  regardless of what it touched. Replaced the broken link with plain text.

Fix `write_routine_fails_on_gitignore_write_error` to actually exercise the `.gitignore` write-failure branch it claims to cover, instead of accidentally failing one line earlier on the `prompts/` subdir creation.

### Fixed

- **Corrected the `stop_json` doc comment's stale claim.** It said `stop --json`'s shape matches
  `status --json` "exactly", but `status --json` later gained `uptime_secs`/`version` fields that
  `stop --json` never got — the two shapes are a subset relationship (already enforced by
  `status_and_stop_json_share_a_common_key_set`), not an exact match. Doc-only; no behavior change.

Fix the routines UI failing to load with `missing field \`prompt\`` (#849) by adding `#[serde(default)]` to `Routine::prompt`, matching the server's `GET /routines` response which omits `prompt` by default since #825.

### Fixed

- **`moadim start` (foreground) could clobber an already-running daemon.** Running `moadim start` in the foreground while a background daemon was already up used to proceed anyway instead of failing fast. It now preflights with `ensure_not_running_for_foreground()` and exits with a clear error before binding, matching the existing background-start behavior.

### Fixed

- **Structurally guarded the routine-launch `sh` spawn against test builds.**
  `spawn_routine_command` invoked `Command::new("sh")` directly, isolated in
  tests only by convention (clearing `PATH`), unlike the `crontab_bin()` seam
  (#175). A future test that triggers a routine without clearing `PATH` could
  execute a real login shell — and thus a real agent launch — on the
  developer's machine. Added `sh_bin()`, mirroring `crontab_bin()`: honors a
  `MOADIM_SH_BIN` override, and in test builds defaults to a nonexistent path
  when no override is set so the spawn fails harmlessly regardless of `PATH`
  state. (#217)

### Added

`GET /health`'s `dependencies` now also reports `python3` (alongside the existing `tmux` flag), and the daemon logs a startup warning when it is missing. The built-in `claude` agent's `setup` step depends on `python3` to pre-seed workspace-trust state; previously a missing `python3` failed that step silently, with the routine still showing a healthy status.

### Fixed

- **A panicking HTTP handler no longer resets the connection with no response.**
  Added `tower_http::catch_panic::CatchPanicLayer` as the outermost layer of
  the Axum router, so an unexpected panic inside a handler now yields a plain
  `500 Internal Server Error` response instead of the client seeing a dropped
  connection and the server logging nothing (issue #337).

### Fixed

- **Routine VEVENTs in the `.ics` feed now carry a `DURATION`.** RFC 5545 requires a `VEVENT` to specify either `DTEND` or `DURATION`; without one, calendar clients rendered each fire as a zero-length instant. Every fire now emits `DURATION:PT15M`.

### Fixed

- **`GET /routines/{id}.ics` no longer panics on a poisoned routine store
  lock.** `svc_ical_routine` locked the shared `RoutineStore` with
  `.lock().expect("routine store lock poisoned")`, unlike its sibling
  `svc_ical` (and every other store accessor) which already recovers via
  `LockRecover::lock_recover()`. Since the store is a process-wide singleton,
  any earlier panic while the lock was held anywhere in the daemon would
  permanently poison it, and this one remaining call site would then panic
  on every subsequent request to the per-routine iCal feed instead of
  degrading gracefully like the rest of the API surface. Switched it to
  `lock_recover()` to close that gap.

Add test coverage for the `X-MICROSOFT-CDO-BUSYSTATUS:FREE` hint emitted alongside `TRANSP:TRANSPARENT` on routine iCal `VEVENT`s (#461).

Move logging setup (`MOADIM_LOG_FORMAT`) into `src/logging/` module folder (`mod.rs` + `tests.rs`), matching the existing `src/utils/`/`src/paths/` convention. Pure file move, no behavior change (#852).

### Fixed

- **Workbench retention was measured from run trigger time, not finish time.** `effective_ttl_secs` is meant to keep a finished run around "only until the next run is due", but measuring age from the trigger timestamp subtracted the run's own duration from its retention window — a run whose duration exceeded its TTL was reaped on the very next sweep, sometimes seconds after completion. Retention is now based on when the run actually finished (`agent.log` mtime, clamped to at least the trigger time). (#174)

### Fixed

- **Renaming a routine no longer strands its prior run history under the old slug.** Workbenches (`~/.moadim/workbenches/{slug}-{ts}`) are keyed by a routine's title slug, not its stable id. `PATCH`/`PUT /routines/{id}` now migrates every existing `{old_slug}-{ts}` workbench to `{new_slug}-{ts}` when the title changes, so `GET /routines/{id}/logs` keeps finding prior runs and the cleanup watchdog keeps resolving an in-flight run to the renamed routine's own `ttl_secs`/`max_runtime_secs` instead of falling back to orphan defaults (#267).

### Fixed

- **OpenAPI spec version now tracks the crate version.** The generated OpenAPI
  document previously reported a frozen `0.1.0`, regardless of the actual
  `moadim` release. It now derives its `info.version` from `CARGO_PKG_VERSION`
  at build time. (#309)

### Fixed

- **Pinned README `--json` shapes to actual CLI keys.** Added tests that parse
  the documented `status`/`cleanup`/`stop` `--json` shape literals straight out
  of `README.md` and assert they name exactly the keys the CLI emits, so a
  field rename, addition, or removal in `cli.rs` can no longer drift silently
  from the script-facing contract. (#345)

### Fixed

- **A failed routine launch left no trace anywhere.** The generated crontab line ran the prompt copy, the agent's `setup` step, and the `tmux` launch with no output redirection, so a failure in any of them (a `setup` error, `tmux new-session` failing, `PATH` not resolving `tmux`) went to cron's mail spool — silently discarded on the headless hosts this daemon targets, leaving no log to read next to the run's other artifacts. Everything after the workbench is created now runs inside a `{ … } >> "$WB/launch.log" 2>&1` group, so these failures are captured in the run's own workbench alongside `agent.log`. (#375)

Remove the `fs_location` middleware (issue #356) that injected `x-server-root` / `x-server-exe-dir` headers, containing the daemon's absolute working-directory and executable paths, into **every** HTTP response. Nothing consumed these headers — the CLI reads JSON response bodies, and the shipped UI has zero references to them — so they were pure information-disclosure surface (OS username + filesystem layout) with no functional dependent. The same `FsLocation` data remains available to intentional callers via `GET /api/v1/health` and the MCP `health` tool.

When renaming this machine via `PUT /api/v1/machine`, automatically update all routines that targeted the old name: replace the old machine name with the new one in each routine's `machines` list, persist the changes to disk, and re-sync the crontab. Previously only the machine identity file was updated, leaving routines orphaned on the renamed machine until each was manually edited.

Echo each request's log correlation id back as an `x-request-id` response header (`src/middlewares/logger.rs`), reusing an inbound `x-request-id` when the caller supplies one instead of always minting a fresh counter-based id. Completes the remaining acceptance criterion of issue #354; the shared inbound/outbound log correlation itself already shipped.

### Added

- **`moadim restart --json`.** Emits the PID-rotation summary as a
  machine-readable `{"old":N|null,"new":M}` object instead of the
  human-readable `restarted: pid <old> -> <new>` line, mirroring the
  `status`/`cleanup`/`stop` `--json` contract.

### Added

- **Optional `goal` for routines.** A routine can now carry a very short (at most
  5 lines) statement of its goal — the "why" behind the prompt. It is optional
  (default unset), persisted in the tracked `routine.toml`, and rendered into the
  agent's `prompt.md` as a `## Goal` preamble ahead of the task. Settable across
  every surface: REST (`goal` on the create/update bodies), MCP
  (`create_routine`/`update_routine`), the CLI (`--goal` on
  `routines create|replace|update`), and the web UI (a field in the routine
  form). The value is trimmed; a goal longer than 5 lines is rejected with
  `400 Bad Request`, and sending an empty string on update clears it. The three
  built-in default routines now ship with a goal. (#827)

### Added

- **Optional per-routine model override.** `Routine`, `CreateRoutineRequest`,
  `UpdateRoutineRequest`, persisted `RoutineToml`, and the MCP
  `UpdateRoutineInput` all gain a `model: Option<String>` field, blank/whitespace
  normalized to `None` (agent's own default). `build_routine_command` appends
  `--model <id>` (shell-quoted) to the agent invocation when set, after the
  agent's own args so it wins over any default. Defaults reconciliation treats
  `model` as user-owned, like `tags`: never overridden by a built-in routine's
  spec. Scoped to the data/API layer for now; the web UI form field is a
  follow-up. (#742)

### Changed

- **A routine's prompt no longer lives inside `routine.toml`.** The raw prompt
  is now stored in its own file, `prompts/prompt.pure.md`, and the composed
  prompt (repositories preamble + raw prompt) moved from the top-level
  `prompt.md` to `prompts/prompt.compiled.md` — both inside the routine's
  directory. Embedding a long, often multi-line prompt as an escaped TOML
  string made `routine.toml` awkward to diff and edit; giving the raw prompt
  its own markdown file finishes the split the daemon already started for the
  composed prompt. Existing installs are migrated automatically on the next
  startup.

### Added

- **Routine snooze.** A new `snooze_routine` MCP tool lets an agent skip its own
  upcoming *scheduled* (cron) fires — either until an absolute unix timestamp
  (`snoozed_until`) or for a fixed count of upcoming fires (`skip_runs`) —
  without touching `enabled`, the crontab, or manual triggers. A snoozed fire
  is skipped before any workbench is spawned; `snoozed_until` clears itself
  once elapsed and `skip_runs` decrements to zero, at which point the routine
  fires normally again. Manual triggers (`trigger_routine`, the UI button)
  always bypass snooze. The Routines table shows a `SNOOZED` badge for
  affected routines.

### Added

- **SUBSCRIBE button on the routines calendar.** The calendar view's nav bar
  now has a SUBSCRIBE button that copies the `/api/v1/routines.ics` feed URL
  to the clipboard, so wiring the feed into an external calendar app no
  longer requires reading the API docs to find the endpoint.

Trim a routine's title before persisting it, so padded input no longer leaks surrounding whitespace into `routine.toml`, workbench `CLAUDE.md` disclosures, and the UI.

Self-host the `Share Tech Mono` webfont (base64-embedded `@font-face` in `ui/index.html` / `prebuilt.html`) instead of fetching it from `fonts.googleapis.com`/`fonts.gstatic.com` at runtime. The served UI now renders offline, with no third-party requests on load, and no FOUT while the CDN round-trip completes (#467). Font is SIL OFL 1.1 licensed; see `ui/assets/share-tech-mono.OFL.txt`.

Add tests for the 8 previously-uncovered `AppError::Internal` error branches in `src/routines/service.rs` (`svc_update`'s goal validation, `svc_trigger_scheduled`'s snooze/skip-runs write paths, `svc_snooze`, `svc_create_flag`, and `svc_resolve_flag`), closing `service.rs` to 100% region coverage. Test-only, no behavior change.

### Fixed

A routine whose composed prompt (prompt + repositories preamble + accumulated open flags) exceeded the OS per-argument limit (Linux `MAX_ARG_STRLEN`, 128 KiB) previously failed to launch with a silent, unreported `execve` error inside the detached tmux session — the run's health dot stayed green with no indication anything went wrong. This only affected agents (like the shipped `claude` default) whose config inlines the prompt via the `{prompt}` placeholder; `{prompt_file}`-based agents (`codex`, `hermes`) were never affected. The daemon now detects an oversized composed prompt before launching and skips the spawn with a visible warning instead.

### Added

- **`ETag` + `304 Not Modified` for the web UI.** `GET /` (and the SPA fallback
  for client-routed paths) now sends a strong `ETag` for the embedded ~1.1 MB
  `index.html`, and honors a matching `If-None-Match` with a bodyless `304`
  instead of re-sending the full body on every load/refresh. `Cache-Control:
  no-cache` keeps the browser revalidating on each request rather than trusting
  a local TTL, since the content can change on any daemon upgrade. (#401)

### Tests

- Added a `cli_tests` regression guard (`status_and_stop_json_share_a_common_key_set`)
  asserting that every object key `stop --json` emits also appears in
  `status --json`, so the shared `{running,pid,address}` base contract between
  the two `--json` shapes can't silently drift apart as fields are added to
  one side but not the other.

### Added

- **`moadim status --wait[=SECS]`.** Polls `GET /health` every 200ms until a
  server answers or `SECS` elapse (default 30) instead of checking once, so a
  launch script can block on startup (`moadim && moadim status --wait`) rather
  than sleeping a fixed guess before probing. Exits `0` once reachable and the
  existing `3` on timeout, matching the `status`/`cleanup`/`stop` exit-code
  contract.

### Changed

- **Closed the Linux systemd-service test gap that was failing `cargo llvm-cov (100% line floor)` on every PR.** `service::linux` (systemd user-unit install/uninstall) had no test seam for `systemctl` and almost no tests, so on the Linux CI runner it sat at ~17% line coverage while `service::macos` (fully seamed and tested) sat at 100% — tripping the repo-wide 100%-line floor and blocking merges regardless of what a PR actually touched. Added a `MOADIM_SYSTEMCTL_BIN` seam mirroring macOS's `MOADIM_LAUNCHCTL_BIN`, split `unit_path()` into a directly-testable `unit_path_from_config_dir()`, and added install/uninstall/write-unit tests mirroring the existing macOS coverage. No behavior change on either platform.

### Tests

- **Covered `routines::flags`'s I/O error paths.** `create_flag`,
  `list_flags`, and `resolve_flag` each have a filesystem-error branch
  (a failed `create_dir_all`, `read_to_string`, or `remove_file`) that
  `cargo llvm-cov` region coverage showed had zero executions. Added three
  tests exercising each: `create_flag_propagates_create_dir_failure`,
  `list_flags_skips_entries_it_cant_read_as_text`, and
  `resolve_flag_propagates_remove_failure`. No behavior change — this locks
  the existing error handling in against a future regression.

### Changed

- Made the web UI's fixed-pixel dimensions fluid: command palette, modals,
  confirm dialog, filter input, and calendar/day nav widths now use
  `clamp()` instead of a single fixed width, and the schedule heatmap
  shrinks its cell/label sizing under 640px instead of relying only on
  horizontal scroll.

### Fixed

- **`slugify` dropped every non-ASCII character.** Routine titles written in
  Hebrew, CJK, or Cyrillic (or Latin letters with diacritics like `é`/`ü`)
  slugified to an empty string and fell back to the generic `"routine"` name,
  so a second such routine collided on create (`409`) and the on-disk
  workbench dir / tmux session name gave no hint which routine it belonged
  to. `slugify` now uses `char::is_alphanumeric`/`char::to_lowercase` (Unicode
  scalar values, not ASCII-only), so non-Latin titles keep their content and
  two distinct non-Latin titles produce distinct slugs. (#262)

Reject empty/whitespace-only entries in a routine's `machines` targeting list on create and update, and trim + dedupe the accepted entries. Previously an unvalidated entry (e.g. `""` or `" host "`) could never match `machine::targets`' exact-string comparison, silently excluding the routine from every machine — and a list of only empty strings slipped past the dormant-routine warning entirely, since that check only fires on an empty list (#600).

Warn when the running daemon binary drifts from the one on disk after an in-place upgrade (#167).

Widen restart-test timing margins to stop flaking under `cargo llvm-cov`.

### Fixed

- **Workbench launch path now derived from `paths::workbenches_dir()`.** The
  generated cron launch command hardcoded `WB="$HOME/.moadim/workbenches/$SLUG-$TS"`
  instead of going through the same seam the reaper (`routines/cleanup/mod.rs`)
  and the LOGS view (`routines/service.rs`) already use. With
  `MOADIM_HOME_OVERRIDE` set, this meant a run was *launched* under one path but
  *reaped and listed* under another — leaking workbenches the reaper never sees
  and leaving the LOGS view empty for real runs. The launch command now resolves
  its base through `paths::workbenches_dir()`, with a regression test asserting
  the two stay in sync under the override. No behavior change for the default
  install. (#601)

## [0.20.0] - 2026-07-02

Enable `clippy::match_same_arms` and merge the two duplicate-body arms it flagged in `cli::parse` (issue #719): the bare `None` arm into the `Background` arm, and the redundant explicit `-h`/`--help`/`help` arm that the trailing wildcard already covered.

### Added

- **CI now enforces `cargo test` and the 100% line-coverage gate.** Previously
  CI only ran fmt + clippy (`lint.yml`); `cargo test` and
  `cargo llvm-cov --fail-under-lines 100` lived solely in the local pre-push
  hook, so a PR from a fork (or from a contributor who skipped hook setup)
  could break tests or drop coverage and still go green. A new `coverage` job
  in `test.yml` mirrors the pre-push hook's `cargo-llvm-cov` invocation
  exactly, keeping the local gate and CI in lockstep. (#150)
- **Routine flags.** A routine's agent runs unattended inside tmux with no
  channel back to a human — until now. It (or a human, via MCP/HTTP) can
  raise a flag against a routine: a free-text `type` (e.g. `"bug"`, `"gap"`,
  `"edge_case"`, `"question"`) and free-text `description`, stored as
  `general` (committed) or `local` (gitignored) under the routine's
  `flags/` folder. New MCP tools `create_flag`, `list_flags`, `resolve_flag`
  and matching `/api/v1/routines/{id}/flags` REST endpoints. Open flags are
  injected into the routine's `prompt.md` on the next run so the agent sees
  what it flagged before, and the UI shows a flag-count badge with a
  read-only flags page to review and resolve them.

- **Structured JSON logging.** Set `MOADIM_LOG_FORMAT=json` to switch `daemon.log`
  (and foreground stdout) from `env_logger`'s human-readable format to one JSON
  object per line (`ts`, `level`, `target`, `msg`), so a `launchd`/`systemd`-run
  daemon can ship its log into an aggregator (Loki, ELK, Vector, CloudWatch)
  without regex-scraping free-form text. Opt-in — the variable unset keeps the
  current text format byte-for-byte, and `RUST_LOG` level filtering is unchanged
  in both formats. (#416)

### Changed

- **Hardened the dashboard's Content-Security-Policy.** Every response's CSP
  previously carried only `frame-ancestors 'none'` (#406's anti-clickjacking
  fix), leaving `script-src`/`style-src`/`default-src` unset and an injected
  inline `<script>` or `<base>` tag entirely unblocked — a real gap given the
  dashboard drives an unauthenticated loopback API with destructive controls
  (create/trigger/delete routines, `POST /shutdown`). The CSP now sets
  `default-src 'self'` and explicit `script-src`, `style-src`, `font-src`,
  `img-src`, `connect-src`, `base-uri 'none'`, `form-action 'none'`, and
  `object-src 'none'` directives verified against the bundled Yew/WASM SPA and
  Swagger UI, while keeping `frame-ancestors 'none'`. (#551)

### Fixed

- **Default routines with empty `machines` list now self-repair.** Default routines
  seeded before machine-awareness was introduced could be left permanently dormant
  (empty `machines` list, so no machine ever matched them). The daemon now detects
  an empty machines list during the startup reconcile pass and seeds the current
  machine, restoring the routine to an active state without any manual intervention.
  (#723)

- The OpenAPI spec (`GET /api/v1`'s `info.version`, the Swagger UI, and the
  committed `apis/openapi.json`) no longer advertises a frozen `0.1.0`. The
  hardcoded `version` literal was dropped from the `#[openapi(info(...))]`
  attribute so utoipa derives it from `CARGO_PKG_VERSION`, keeping the spec
  version in lockstep with the crate.

### Tests

- Added a `cli_tests` regression guard (`status_and_stop_json_share_a_common_key_set`)
  asserting that every object key `stop --json` emits also appears in
  `status --json`, so the shared `{running,pid,address}` base contract between
  the two `--json` shapes can't silently drift apart as fields are added to
  one side but not the other. `status --json` may carry additional
  server-sourced fields (`uptime_secs`, `version`) that `stop --json` omits;
  see `status_and_stop_json_share_the_same_shape` for the value-level guard on
  the shared subset.

## [0.19.1] - 2026-07-01

### Fixed

- **The routines page failed to load with "missing field `prompt`".** PR #825
  made `GET /routines` omit the `prompt` field from each routine's JSON by
  default, but the UI's separately-mirrored `Routine` struct never got a
  matching `#[serde(default)]` on that field, so the wasm client's
  deserialization broke on every list fetch.

### Added

- **Repository filter for the Routines table.** The REST `GET /routines`
  endpoint has supported a `?repository=` filter for a while, but the web UI
  had no way to use it — the only client-side facets were status, agent, and
  machine. Added a REPOSITORY dropdown to the Routines filter bar (mirroring
  the existing agent/machine facet pattern), populated from the distinct
  repository URLs across loaded routines, so operators can narrow a dense
  routines list to a single repo without hand-editing the query string.

## [0.19.0] - 2026-07-02

### Removed

- **Removed the cron-job feature.** Moadim scheduled two kinds of things —
  "cron jobs" (a schedule + a handler script) and "routines" (a schedule + an
  AI-agent prompt). The project's focus is AI-agent routines, so the cron-job
  half — the `CronJob`/`CronStore` model, the `/api/v1/cron-jobs*` REST routes,
  the `*_cron_job` MCP tools, the `moadim cron-jobs` CLI subcommand, the
  `~/.config/moadim/jobs/` and `~/.config/moadim/handlers/` directories, and
  the job-specific crontab block — has been removed. Routines are unaffected
  and keep their own crontab block, REST routes, MCP tools, and CLI
  subcommand. (#842)

### Changed

- **`list_routines` omits routine prompts by default.** The prompt is the
  largest field on a routine and is rarely needed when scanning a listing, so it
  bloated `GET /routines` responses and burned MCP context tokens on every call.
  The `prompt` key is now absent from list entries unless the caller opts in with
  `include_prompts=true` (a new boolean on the `list_routines` MCP tool and the
  `GET /routines` query string). `get_routine` / `GET /routines/{id}` are
  unaffected and always return the prompt; `routine.toml` persistence is
  unchanged. (#824)

### Documentation

- **Added `CODE_OF_CONDUCT.md`.** The repo had a `CONTRIBUTING.md` and
  `SECURITY.md` but no documented standard of conduct or enforcement contact,
  leaving GitHub's community-standards profile incomplete. Added a Contributor
  Covenant v2.1 code of conduct with a real reporting contact, and linked it
  from `CONTRIBUTING.md`. (#423)
- The README's **Bind address** section now warns that the REST API and MCP
  endpoint are unauthenticated, so `MOADIM_BIND_ADDR` should stay on a loopback
  address: binding to a routable interface (a LAN IP or `0.0.0.0`) exposes
  unauthenticated routine create/trigger — effectively remote code execution — to
  the network. Recommends an authenticating reverse proxy / firewall for remote
  access instead. (#253)

### Fixed

- **The committed `prebuilt.html` fallback UI was stale and silently missing
  features.** `build.rs` inlines the compiled Yew UI into `prebuilt.html` and
  falls back to that committed copy whenever `trunk` isn't installed at build
  time (e.g. `cargo install moadim` from a git checkout, or a Docker build
  without trunk) — but nothing verified it stayed in sync with `ui/src`. It had
  drifted: the copy on `main` was missing the machine-name badge, machine
  filter, and rename-machine dialog entirely, even though that code had long
  since landed. Regenerated `prebuilt.html` and added a `prebuilt-ui` CI job
  that rebuilds it with trunk on every PR touching `ui/` and fails if the
  committed copy doesn't match, so this can't silently regress again.
- **Reaped workbenches no longer leak a `~/.claude.json` `projects` entry.** The built-in `claude`
  agent's `setup` step seeds a per-workbench entry into `~/.claude.json`, keyed by the workbench's
  absolute (always-unique) path, on every run. Nothing ever pruned it once the workbench was reaped,
  so the file grew by one dead entry per `claude` run, forever. Cleanup now removes the matching
  `projects[<workbench>]` entry when it reaps a workbench directory, using the same flock-guarded
  read -> modify -> atomic-replace pattern the setup step already uses. (#430)
- **Workbench launch path now derived from `paths::workbenches_dir()`.** The
  generated cron launch command hardcoded `WB="$HOME/.moadim/workbenches/$SLUG-$TS"`
  instead of going through the same seam the reaper (`routines/cleanup/mod.rs`)
  and the LOGS view (`routines/service.rs`) already use. With
  `MOADIM_HOME_OVERRIDE` set, this meant a run was *launched* under one path but
  *reaped and listed* under another — leaking workbenches the reaper never sees
  and leaving the LOGS view empty for real runs. The launch command now resolves
  its base through `paths::workbenches_dir()`, with a regression test asserting
  the two stay in sync under the override. No behavior change for the default
  install. (#601)
- **The daemon never killed hung routine tmux sessions when launched via
  launchd/systemd.** Those managers start `moadim --interactive` with a
  minimal `PATH` (e.g. macOS launchd's `/usr/bin:/bin:/usr/sbin:/sbin`) that
  hides a Homebrew- or npm-installed `tmux`. The daemon's own cleanup/watchdog
  sweep shelled out to `tmux` directly (no login shell), so every
  liveness/kill probe silently failed and read as "session already dead" —
  a hung run's workbench got TTL-reaped while its real tmux session and agent
  process kept running, untracked, forever. `resolve_tmux_bin` now also
  searches common install locations (Homebrew, `/usr/local/bin`,
  `~/.local/bin`) when `tmux` isn't on `PATH`, and the generated launchd
  plist now sets a real `PATH` via `EnvironmentVariables`.
- **`cargo build` was broken on `main`.** Two independent PRs (#804 and #805)
  each added a `unused_async = "deny"` entry under `[lints.clippy]` in
  `Cargo.toml`, and both merged cleanly since git's line-based merge doesn't
  understand TOML semantics. The resulting duplicate key made every `cargo`
  invocation fail immediately with `error: duplicate key` before compiling a
  single crate. Removed the duplicate entry so the workspace builds again.
- `docs/moadim.1`'s `.TH` header reported a stale `moadim 0.16.0` even though
  `Cargo.toml` had moved on to 0.18.0 — the hand-maintained man page has no
  build-time link to the crate version, so a release could silently ship a man
  page reporting the *previous* version. Corrected the version token and added
  a regression test (`cli::cli_tests::man_page_version_matches_cargo_pkg_version`)
  that fails when the two drift again. (#556)
- **`slugify` dropped every non-ASCII character.** Routine titles written in
  Hebrew, CJK, or Cyrillic (or Latin letters with diacritics like `é`/`ü`)
  slugified to an empty string and fell back to the generic `"routine"` name,
  so a second such routine collided on create (`409`) and the on-disk
  workbench dir / tmux session name gave no hint which routine it belonged
  to. `slugify` now uses `char::is_alphanumeric`/`char::to_lowercase` (Unicode
  scalar values, not ASCII-only), so non-Latin titles keep their content and
  two distinct non-Latin titles produce distinct slugs. (#262)
- Locked the `--json` machine-readable contract with a regression test
  (`cli::cli_tests::status_stop_cleanup_json_share_the_same_address`) asserting
  `status --json`, `stop --json`, and `cleanup --json` all surface the same
  `address` value, so the three shapes can't silently drift apart again. (#245)
- Removed two dead `AppError::NotFound` arms in `svc_update` (`routines/service.rs`):
  the function already checks the routine's existence once, up front, while
  holding the store's lock continuously for the rest of the call, so the two
  later re-fetches could never actually miss. The two tests written to cover
  those unreachable arms were accidental duplicates of the same first-check
  path; merged them into one `svc_update_not_found_when_id_missing` test that
  covers both request shapes against the real, single `NotFound` path.
- Nothing previously verified the README's documented `--json` object shapes
  for `status`/`cleanup`/`stop` against the keys the CLI actually emits, so a
  field renamed, added, or removed in `cli.rs` could drift silently from the
  script-facing contract in `README.md`. Added
  `cli::cli_tests::readme_status_json_shape_matches_actual_keys` and its
  `cleanup`/`stop` counterparts, which parse the documented shape literal
  straight out of `README.md` and assert it names exactly the keys
  `status_json`/`cleanup_json`/`stop_json` produce. (#345)

### Added

- **`moadim restart -i`/`--interactive`.** `restart` previously always spawned
  the fresh instance detached in the background, with no way to bring it up
  attached to the terminal in one step (mirroring `moadim -i`). The new flag
  stops the running server (if any) and starts the replacement in the
  foreground instead of backgrounding it.
- **`ETag` + `304 Not Modified` for the web UI.** `GET /` (and the SPA fallback
  for client-routed paths) now sends a strong `ETag` for the embedded ~1.1 MB
  `index.html`, and honors a matching `If-None-Match` with a bodyless `304`
  instead of re-sending the full body on every load/refresh. `Cache-Control:
  no-cache` keeps the browser revalidating on each request rather than trusting
  a local TTL, since the content can change on any daemon upgrade. (#401)
- **SUBSCRIBE button on the routines calendar.** The calendar view's nav bar
  now has a SUBSCRIBE button that copies the `/api/v1/routines.ics` feed URL
  to the clipboard, so wiring the feed into an external calendar app no
  longer requires reading the API docs to find the endpoint.
- **`moadim status --wait[=SECS]`.** Polls `GET /health` every 200ms until a
  server answers or `SECS` elapse (default 30) instead of checking once, so a
  launch script can block on startup (`moadim && moadim status --wait`) rather
  than sleeping a fixed guess before probing. Exits `0` once reachable and the
  existing `3` on timeout, matching the `status`/`cleanup`/`stop` exit-code
  contract.
- **Escape dismisses open UI modals/dialogs.** The shutdown-confirm and
  rename-machine dialogs and the routine edit/delete-confirm modals now all
  close on `Esc`, matching the command palette's existing behavior.
- An interactive foreground start (`moadim -i` / `--interactive`) now preflights
  for an already-running daemon and refuses with a clear, actionable message
  (naming the running pid when known and pointing at `moadim stop` /
  `moadim restart`) instead of proceeding to bind and dying with an opaque
  `Address already in use (os error 48)`. The launcher-spawned background child
  (which also runs `--interactive`) is exempt via the `MOADIM_DAEMONIZED` marker,
  so background/restart launches are unaffected (#298).
- **Machine name in health output.** `GET /health` and the MCP `health` tool now
  report the daemon's resolved machine identity (from `MOADIM_MACHINE`,
  `machine.local.toml`, or hostname — same as `GET /machine`) in a new `machine`
  field, so clients can tell which machine answered without a second request. (#778)
- **`agent_command_available` on routine responses.** `RoutineResponse` (returned
  by `GET`/`POST`/`PUT`/`DELETE` `/routines`) now reports whether the routine's
  agent `command` (e.g. `claude`, `codex`) actually resolves on the daemon's
  `PATH`, distinct from the existing `agent_registered` (which only checks that
  `<agent>.toml` exists). A routine with a present, well-formed agent config but
  an uninstalled binary previously looked identically healthy to one that could
  actually run — the cron firing launches a tmux session that dies immediately
  with "command not found," a silent no-op. Clients can now tell the two states
  apart instead of inferring it from `agent.log` after the fact. (#383)
- **`actionlint`/`shellcheck` CI gate.** New `.github/workflows/actionlint.yml`
  runs `actionlint` (via `raven-actions/actionlint`, pinned to a commit SHA) on
  every PR and on push to `main`, statically validating workflow YAML —
  syntax, `${{ }}` expressions, the `needs`/`if`/matrix job graph, event
  triggers, action input names — and, with `shellcheck` enabled by default,
  linting every embedded `run:` block. Previously a typo'd key, a bad
  expression, or an unquoted shell variable in `.github/workflows/` only
  surfaced when the workflow actually ran on `main` or a release tag.
  Documented in `CONTRIBUTING.md` alongside the other lint tooling. (#454)

### Changed

- Enabled the `clippy::wildcard_imports` lint. It flags `use some::module::*;`
  glob imports, which obscure where a name comes from at the call site, can
  silently change behavior when the globbed module gains a new item, and
  defeat "go to definition" tooling. Zero existing violations, so this only
  guards against the pattern creeping in going forward. No behavior change.
- Enabled the `clippy::unused_async` lint. It flags `async fn`s (and async
  closures/blocks) that never `.await` anything internally, which needlessly
  propagate async-ness up the call stack and pull in a `Future` state machine
  for work that's actually synchronous. Zero existing violations, so this only
  guards against the pattern creeping in going forward. No behavior change.
  (#803)
- **Gzip-compressed HTTP responses.** The Axum router now negotiates
  `Accept-Encoding` and gzip-compresses response bodies via a `tower-http`
  `CompressionLayer`, cutting the ~1.1 MB SPA payload (and the OpenAPI JSON
  under `/docs`) several-fold on every load/refresh for clients that
  advertise gzip support. A no-op for clients that don't. (#399)
- Bumped `tower-http` from `0.6.11` to `0.7.0`. No source changes needed —
  the only feature used (`compression-gzip`'s `CompressionLayer`) is
  API-compatible across the bump.
- Declared `rust-version = "1.88"` (MSRV) in the root `Cargo.toml` and
  `ui/Cargo.toml`, matching the floor already required transitively by
  `darling 0.23`, so `cargo install moadim` on an older stable toolchain now
  fails with Cargo's clean MSRV message instead of an opaque compile error. (#326)
- The local pre-push hook now runs `cargo clippy --workspace` instead of a
  bare `cargo clippy`. In this non-virtual workspace (the root `Cargo.toml`
  declares both a `[package]` and `[workspace]`), the bare form only checks
  the root `moadim` package, so the `ui` member crate was never type-checked
  or linted by the hook.
- `.github/workflows/lint.yml`'s `clippy` job now runs `cargo clippy
  --workspace --all-targets -- -D warnings` too, closing the matching gap in
  CI: previously the bare `cargo clippy --all-targets` only checked the root
  `moadim` package, so `ui/Cargo.toml`'s `[lints.clippy] all = "deny"`
  posture was never enforced on PRs and a dashboard lint regression could
  merge to `main` fully green, only surfacing via the local hook (if a
  contributor had it installed) or at release time.
- `build_app_with_shutdown` cloned `store` and `routines` into `app_state`,
  then cloned them *again* from the original bindings for the MCP service
  closure — `clippy::redundant_clone` flags the second pair as dead clones
  since the originals are never read afterward. Reordered to clone once
  (for the MCP closure) before moving the originals into `app_state`,
  dropping two unnecessary `Arc` clone+drop pairs per router build. No
  behavior change.

### Fixed

- Fixed the `ui` crate, which had silently stopped compiling: three test
  fixtures (`command_palette_tests.rs`, `routines_tests.rs`,
  `schedule_heatmap_tests.rs`) were missing the `tags` field added to
  `Routine` by #505, and `cron_jobs::unassigned_count` /
  `routines::unassigned_routines_count` were dead code left over from before
  #771 made the "Unassigned" machine facet a permanent filter option. None of
  this was caught because `ui` was outside the pre-push clippy gate and CI's
  equivalent gate had the same blind spot (now closed, see `--workspace`
  fix above).

### Fixed

- **Flaky `restart` test under `cargo llvm-cov`.** `stop_running_and_wait_succeeds_without_pid_file_when_server_eventually_stops`
  used a 60ms timeout against an 80ms fake-server stop delay, leaving margins too
  tight to survive the scheduling jitter and slowdown that coverage
  instrumentation adds, so it intermittently failed the pre-push 100%-coverage
  gate. Widened to a 300ms timeout against a 450ms stop delay so the same
  code path is still exercised with headroom to spare.

## [0.18.0] — 2026-06-30

### Added

- **Optional `tags` for routines.** Routines can now carry a free-form list of
  string labels for grouping and organization. Tags are optional (default empty),
  persisted in the tracked `routine.toml`, and settable across every surface: REST
  (`tags` on the create/update bodies), MCP (`create_routine`/`update_routine`),
  the CLI (repeatable `--tag` flag on `routines create|replace|update`), and the
  web UI (a comma-separated field in the routine form plus a column in the table).
  Blank or whitespace-only tags are rejected with `400 Bad Request`. (#502)

## [0.17.1] — 2026-06-29

### Changed

- Machine filter in Routines and Cron Jobs views now always shows a **None** option
  (routines/jobs with no machine assigned), replacing the previously conditional
  "Unassigned" entry that only appeared when such items existed.

### Fixed

- `~/.config/moadim/.gitignore` required patterns (`*.pid`, `*.log`,
  `*.local.*`) are now ensured on every daemon start instead of only when
  the file is absent, so a manually edited or newly required entry is
  restored automatically. (#770)

## [0.17.0] — 2026-06-29

### Added

- Machine name badge in the header: the resolved `@ <name>` is shown as a
  clickable chip. Clicking it opens a rename dialog that calls the new
  `PUT /api/v1/machine` endpoint, writes the new name to
  `machine.local.toml`, and updates the badge immediately on success.
  Empty names are rejected (400). (#766)

- `moadim uninstall` now clears the managed crontab blocks (both
  `# BEGIN MOADIM-ROUTINES` and `# BEGIN MOADIM`) in addition to removing the OS
  service, so `cron` stops firing routines/jobs against a daemon you uninstalled.
  The routines block is cleared first because its marker is a superstring of the
  cron-jobs marker (avoids the #324 collision). Best-effort and idempotent — a
  crontab with no managed block, no crontab at all, or a failed service-removal
  step still completes the cleanup — and it reports how many managed entries were
  removed. (#380)

- `GET /health` now reports a `dependencies` section (currently `{"tmux": bool}`)
  so the UI/CLI can detect when the `tmux` runtime dependency is missing, and the
  daemon logs a `warn!` at startup naming the missing binary. `tmux` is a hard
  dependency — every routine agent launches inside a tmux session — but its
  absence was previously unchecked and undocumented, so a host without `tmux`
  made scheduled routine runs silently no-op. Detection reuses the existing
  PATH probe (`tmux_available_in` / `tmux_available`) (#187).

- `GET /routines.ics` accepts an optional **`?routine=<id>`** query param that
  scopes the feed to a single routine, so a calendar client can subscribe to one
  routine's fire times instead of the firehose of every routine on the host. The
  filtered calendar is named after the routine (`X-WR-CALNAME`); an unknown or
  disabled id yields a well-formed empty calendar (still `200 text/calendar`).
  Without the param the feed is unchanged — every enabled routine (#263).

- The generated routines crontab block is now deterministic when several
  routines share the same `created_at`. The block is built from a `HashMap`,
  whose iteration order is unspecified, so tied routines previously emitted in
  an arbitrary, run-to-run order — churning the block across syncs and defeating
  the idempotency guard, which forced a needless `crontab -` rewrite that
  mutates the user's live crontab. Ties are now broken on the stable routine id.
- **UI overview: "▶ RUN" quick-trigger button in the Upcoming Runs table.**
  Each row in the UPCOMING RUNS table on the Overview page now carries a
  `▶ RUN` button that fires the job's trigger endpoint
  (`POST /api/v1/routines/{id}/trigger` or `/api/v1/cron-jobs/{id}/trigger`)
  without leaving the page. A toast confirms success or surfaces the error.
  Implements the "quick actions" best practice from CI/CD operations dashboards
  (Cronitor, Temporal, GitHub Actions) where operators can fire jobs directly
  from the at-a-glance view.
- **iCal feed: carriage returns in routine titles/prompts no longer corrupt content lines.**
  `escape_text` now normalises both bare `\r` and CRLF sequences to an escaped newline (`\n`)
  before emitting them into a `TEXT` property value, satisfying RFC 5545 §3.3.11 which forbids
  raw CR characters in content lines. Closes #181.
- A `fmt + clippy` CI workflow (`.github/workflows/lint.yml`) that mirrors the
  pre-push hook (`cargo fmt --check`, `cargo clippy -- -D warnings`) on every PR
  and push to `main`, so style/lint regressions are caught in review without
  relying on local hooks.

### Documentation

- Documented the required external binaries (`tmux`, `crontab`) under a new
  **Prerequisites** section in the README (#187).

### Changed

- The built-in Claude agent now reads its project instructions from `AGENTS.md`,
  the same file Codex uses, unifying the moadim-managed system prompt and
  routine-origin disclosure onto a single instructions file across agents. Claude
  Code loads `AGENTS.md` as a memory/context file, so the disclosure is honored
  exactly as it was from `CLAUDE.md`. User-authored agent configs that omit
  `instructions_file` still fall back to the historical `CLAUDE.md` default.

- The request logger now records `GET /health` at `debug` instead of `info`.
  The web UI polls `/health` continuously, so at the default `info` level those
  two-lines-per-poll entries dominated `daemon.log` (thousands of lines a day on
  an otherwise idle daemon) and buried every other request. Health polls remain
  visible under `RUST_LOG=debug`; all other requests still log at `info`.

- On first run (no `MOADIM_MACHINE` env var and no `machine.local.toml`), the
  daemon now auto-generates a unique machine name (`machine-{8hex}`) and writes
  it to `machine.local.toml` rather than silently falling back to the system
  hostname. A `warn!` log names the generated value and points to
  `moadim machine set <name>` to override it. If the write fails the hostname
  fallback is preserved. Closes #762.

- Enabled the `clippy::map_unwrap_or` lint and fixed the violations
  (`map(...).unwrap_or(...)` → `map_or(...)`). No behavior change.

### Fixed
- iCal feeds now summarize long routine prompts in the DESCRIPTION field (first line, truncated to 120 chars) instead of embedding the full prompt, keeping calendar entries readable. (#185)
- **6-field cron expressions no longer silently fail to fire.** `croner` accepts
  6-field (`sec min hour dom month dow`) and 7-field expressions, but the OS
  crontab only understands 5 fields. Both forms are now normalised to 5 fields
  by dropping the leading seconds (and, for 7-field, the trailing year) before
  the expression is stored or written to the crontab. Previously a 6-field
  string was written verbatim, making the job malformed and silently inactive.
  Closes #183.

- **`moadim stop --json` now reports the real bound `address`.** Under a
  `MOADIM_BIND_ADDR` override, `stop --json` emitted the hardcoded default
  `127.0.0.1:5784` while `status --json` reported the actual address, so the two
  `--json` objects disagreed despite the documented "identical shape" contract.
  `stop_json` now uses `bind_addr()` like `status_json`. Added a regression test
  plus a guard asserting `status`/`stop` produce the same object.

- The pid file is now reconciled against process liveness before it is reported
  or acted on. After a `kill -9`, panic, OOM kill, or power loss the graceful
  shutdown path never runs, so the pid file lingers with a now-dead PID.
  `read_pid_file()` now treats a recorded PID that is not a live process
  (`kill -0` probe on Unix) as absent and cleans the stale file up best-effort.
  `status`/`stop --json` therefore emit `pid: null` consistently with
  `running: false` instead of a dead-or-PID-reused number, and `restart` never
  force-kills a stale PID. (#315)

- The daemon now writes its managed system prompt and routine-origin disclosure to the agent's designated instructions file (`AGENTS.md` for Codex). Previously the Codex agent received the disclosure via a separate mechanism. (#152)

- An agent config that exists on disk but cannot be read (due to a permissions
  error or path collision) is now reported as `AgentLoadError::Unreadable` rather
  than `AgentLoadError::Missing`. Previously, any `read_to_string` failure was
  silently mapped to `Missing`, causing `validate_agent` to accept the broken
  config (it tolerates `Missing` for configs not yet created), leaving a
  green-dot routine that silently never fires. The new `Unreadable` variant is
  rejected at create/update time with a `BadRequest`, so the operator learns the
  real cause immediately. (#445)
- Loading a routine whose `routine.toml` is unparsable or missing a required
  field (title, schedule, or agent) now logs a `warn` naming the directory,
  instead of silently dropping the routine from the store, UI, API, and crontab
  with no trace. Directories with no `routine.toml` are still skipped quietly.
  (#530)
- Build provenance now marks a dirty working tree. A binary built from a tree
  with uncommitted changes to tracked files gets a `-dirty` suffix on its short
  SHA (e.g. `a1b2c3d-dirty`) in `moadim --version`, `GET /api/v1/health`, and the
  MCP provenance, instead of misreporting a clean SHA that doesn't match its
  source. A pristine checkout is unchanged, and the `"unknown"` (no-git) fallback
  is preserved. (#491, follow-up to #367)
- **macOS: TCC "administer your computer" dialog no longer appears during background runs.**
  `moadim install` now proactively sends a harmless Apple Event to System Events so macOS
  prompts for the Automation permission once, while the user is at the terminal. After clicking
  OK the grant is remembered permanently; the background daemon never triggers the dialog again.
  A hint line is printed before the prompt so users know what to expect. Closes #730.

- **Trigger-spawned processes are now reaped so the daemon no longer leaks zombie
  (`<defunct>`) entries.** Both the routine trigger (`POST /routines/{id}/trigger`)
  and the cron-job trigger (`POST /cron-jobs/{id}/trigger`) previously dropped
  the spawned child handle without calling `wait()`, so every trigger leaked one
  zombie for the daemon's lifetime. A new `utils::process::spawn_and_reap` utility
  spawns the command and hands the child to a detached thread that reaps it.
  (#212)

### Added

- **UI: clickable chips in Calendar and Day timeline views.** Schedule chips in the monthly
  Calendar grid and the 24-hour Day timeline now open the edit modal when clicked, on the
  Routines page. Previously these views were read-only; clicking a chip did
  nothing. The chip carries the entity id through `TimelineItem` to the existing
  `on_click` / `on_edit` callback, dispatching the same `OpenEdit` action as the table
  EDIT button. Implements the contextual-click-to-edit pattern from leading scheduling dashboards
  (Airflow calendar view, GitHub Actions timeline, Temporal UI). Closes #746 (see also #728, #748).

- `moadim trigger <id>` triggers a routine to run immediately from the terminal,
  outside its schedule — the same on-demand run the REST API
  (`POST /routines/{id}/trigger`) and the MCP tool already expose. Prints
  `triggered routine <id>` and exits `0` on success, errors with
  `no routine with id <id>` on a `404`, and prints `moadim is not running`
  (exit `3`) when no server is reachable, matching the `status`/`cleanup`
  exit-code contract. (`moadim run <id>` is accepted as a hidden back-compat
  alias.)

- **UI: group-by dimension for the Routines table.** A **GROUP BY** selector in the section
  toolbar lets operators partition the flat routine list into labelled sections by **Agent**,
  **Machine**, or **Status** (Enabled / Disabled), with a **None** option to restore the flat
  view. The selector only appears in Table view (hidden for Calendar and Day) and composes with
  the existing faceted filter and column-sort controls. Closes #733.

- **UI: clone/duplicate a routine.** A ⧉ duplicate button on each routine row opens the
  create-routine form pre-filled with all fields from the source routine (schedule, agent, prompt,
  repositories, machines, TTL, enabled state). The title is automatically prefixed with
  "Copy of " (and the prefix is not doubled on repeated clones). Operators can adjust any field
  before saving; the result is a brand-new independent routine. Closes #715.
- **Local-machine filter for routines and cron jobs.** A new `GET /api/v1/machine` endpoint
  returns the daemon's resolved machine name. `GET /routines` and `GET /cron-jobs` now accept a
  `local_only=true` query parameter that filters the response to entries targeting the current
  machine. The MCP `list_routines` and `list_cron_jobs` tools gain the same parameter, defaulting
  to `true` so MCP callers see local-first results. The UI routines and cron-jobs pages fetch the
  current machine on mount and default the existing machine facet filter to it; users can change
  the filter to "Any" to see all machines. Closes #726.

- **UI: group-by dimension for the Routines table.** A GROUP BY selector in the table toolbar
  lets operators partition the routine list into labelled sections by **agent** (e.g. `claude`,
  `codex`), **machine** (first assigned machine, or `(unassigned)` when none), or **status**
  (Enabled / Disabled). Selecting "None" (the default) restores the existing flat list. The
  selector is hidden in Calendar and Day views where grouping is not applicable. Mirrors the
  identical GroupBy feature on the Cron Jobs page. Closes #744.

## [0.16.0] - 2026-06-26

### Changed

- **`defaults` module split.** `src/routines/defaults.rs` is now a module (`defaults/`); each
  built-in routine lives in its own file (`update_moadim.rs`, `the_1_percent.rs`). Pure
  refactor — no behaviour change.

### Added

- **Per-row health-status badge in the Routines table.** A new sortable **HEALTH** column
  shows a colored badge on every routine row: `HEALTHY` (accent), `DISABLED` (muted),
  `DORMANT` (amber — enabled but no machine assigned), `DEAD SCHEDULE` (red — schedule
  yields no future fire), and `AGENT MISSING` (amber — agent config not registered).
  Badges follow the traffic-light pattern used by Jenkins, GitHub Actions, and Datadog:
  color + text label together so status is legible without color vision. Sorting ascending
  puts the most-urgent rows first (Dormant → Dead Schedule → Agent Missing → Disabled →
  Healthy), letting operators triage broken routines in one click. The **LAST FIRE** column
  header is also now sortable. Pure frontend — no backend change. Closes #712.
- **Group-by dimension for the Cron Jobs table.** A new **GROUP BY** selector in the
  Cron Jobs toolbar lets operators partition the flat job list into labelled sections
  by **Handler**, **Machine**, or **Status** (enabled/disabled). Within each group the
  active column sort still applies; groups are ordered alphabetically for a stable
  layout. `None` (the default) preserves the existing flat-list behaviour. Backed by
  a pure `group_jobs()` / `group_key()` function covered by 16 new host-only tests.
  Follows the first-class grouping pattern in Airflow's DAG list, GitHub Actions
  workflow runs, and Temporal namespace views — orthogonal to filtering so operators
  can filter *and* group simultaneously. Pure frontend — no backend change.
  Closes #714.
- **Dedicated LAST FIRE column in the Routines table.** The most-recent trigger
  timestamp is now shown in its own **LAST FIRE** column directly beside NEXT RUN,
  matching the side-by-side "last run / next run" pattern standard in Airflow, Temporal,
  and Kubernetes CronJob dashboards. A ↻ prefix marks manual triggers; ⏱ marks
  scheduled fires; routines that have never been triggered show `—`. The trigger data
  was already returned by the API — it was previously buried as a sub-line under the
  UPDATED cell where it was easy to miss. Pure frontend — no backend change.
  Closes #660. Closes #688.
- **Schedule fire preview on Cron Jobs and Routines pages.** Every schedule cell now has a
  **▸ fires** toggle button. Clicking it expands an inline panel listing the next 10 scheduled
  fire times for that job or routine (absolute time + relative countdown per entry); clicking
  again collapses it. Implements the per-job forward-projection pattern used by Cronitor,
  BetterStack, and Cloud Scheduler — operators can verify an expression after editing or check
  whether a job falls inside a maintenance window without guessing from the human description.
  Pure frontend: `next_fires(schedule, now, n)` iterates the existing croner iterator and
  collects up to `n` datetimes; no backend change. Closes #704.
- **Calendar view for the Cron Jobs page.** The Cron Jobs page gains a CALENDAR
  view alongside the existing LIST and DAY views, matching the three-view layout
  of the Routines page. Operators can browse a 6-week monthly grid showing how
  many times each enabled job fires per day, with prev/next/today navigation.
  Calendar grid helpers (`WEEKDAYS`, `CAL_MONTHS`, `GRID_CELLS`, `MAX_OCCURRENCES`,
  `month_start`, `occurrences_per_day`) are extracted from `routines.rs` into the
  shared `schedule` module so both pages share the same implementation.
- **Global routine lock — UI banner and REST API.** The Routines page shows an amber banner
  when a global lock is active, listing which sentinel(s) are present (SHARED / LOCAL) with an
  **UNLOCK ALL** button that removes both via `DELETE /api/v1/routines/lock?scope=all`. Three
  new REST endpoints expose lock management: `GET /routines/lock` (status), `POST /routines/lock`
  (create sentinel; scope=shared|local), `DELETE /routines/lock` (remove; scope=shared|local|all).
- **Global routine lock.** Create `~/.config/moadim/.lock` (committed, shared via git) or
  `~/.config/moadim/.local.lock` (gitignored, machine-local) to pause all routine scheduling
  and manual triggers without touching individual routine `enabled` states. Removing the file(s)
  restores prior state. Three new MCP tools — `get_lock_status`, `lock_routines`,
  `unlock_routines` — manage the sentinels and immediately re-sync the crontab. Blocked triggers
  return HTTP 423 Locked.
- **Bulk actions for the Routines page.** Each routine row now has a leading selection
  checkbox; a header checkbox toggles "all visible selected ↔ none" (respects the active
  filter so hidden rows are never touched). When at least one routine is selected, a
  floating bulk-action bar appears with **ENABLE**, **DISABLE**, and **DELETE** actions plus
  a **CLEAR** affordance. Bulk enable/disable fires `PATCH /routines/{id}` for each
  selected routine and surfaces a single summary toast. Bulk delete shows a confirmation
  dialog and removes via `DELETE /routines/{id}`. Selection is automatically pruned on
  reload so stale IDs never carry over. Pure frontend — no backend change. Closes #676.
- **Token Trim default routine.** A new built-in weekly routine (Sundays 07:00) that audits
  routine prompts for redundancy, verbosity, dead scaffolding, and duplication, then opens one
  PR per week that reduces LLM token consumption without degrading output quality.
- **Light/dark theme toggle.** A ☀/🌙 button in the header switches between the dark
  terminal aesthetic and a clean light palette. The choice persists to `localStorage`
  under `moadim.theme` and is applied flash-free via an inline `<head>` script before
  the first paint. The `⌘K` command palette gains a "Toggle Theme" entry so
  keyboard-first operators never need to reach for the mouse. All colours are pure CSS
  custom-property overrides — no per-component changes. Closes #664.
- **Sortable column headers for the Cron Jobs table.** Clicking any column header
  (ID, HANDLER, NEXT RUN, ENABLED, UPDATED) sorts the table by that field; clicking
  again reverses direction. An arrow indicator shows the active sort column and
  direction. Sort state lives in component memory (no URL pollution) and resets to the
  server's natural order on page reload. Pure client-side — no backend change.
  Closes #657, #669.
- **NEXT RUN countdown column in the Routines table.** The Routines table gains a
  live **NEXT RUN** column (absolute fire time + relative countdown + due-soon accent)
  matching the already-shipped column on the Cron Jobs page, so operators see per-routine
  next-fire times at a glance without navigating to the Overview. Disabled routines show
  `paused`; invalid or exhausted schedules show `—`; countdowns turn green inside the
  1-hour due-soon window. A 30 s background tick keeps countdowns live between data
  fetches. Pure client-side computation from the existing `schedule` field — no backend
  change. Closes #653.
- **Cross-filterable KPI tiles + DueSoon facet for Routines page.** The Routines
  page's static stat cards are replaced by clickable `<button>` tiles with
  `aria-pressed`; clicking ENABLED, DISABLED, or DUE SOON applies that status
  filter to the list, and clicking the active tile clears it. A new `DueSoon`
  status facet selects routines whose next scheduled fire lands within the next
  hour (same 1-hour window used by the Cron Jobs page). A live 30-second `now`
  tick keeps the DueSoon count current between data fetches. The STATUS dropdown
  in the filter bar gains a "Due soon" option. The `/` key shortcut focuses the
  search box when the user is not already typing in a field. Closes #652.
- **Enhanced log viewer.** The per-job and per-routine log panel gains line numbers,
  a keyword search bar with match highlighting and navigation arrows, and an
  auto-tail toggle that keeps the viewport pinned to the last line as new output
  arrives. Closes #646.
- **"The 1 Percent" built-in default routine.** A new daemon-managed default that fires
  daily at 08:00 and audits the user's automation portfolio across six dimensions (coverage
  gaps, redundancy, dead weight, prompt quality, schedule hygiene, machine targeting). Each
  run it picks the single highest-impact improvement and opens a pull request on the routines
  repository. If the routines folder is not a git repository the routine self-disables via
  `update_routine`. Closes #640.
- **Fleet schedule heatmap.** A new HEATMAP page (`/heatmap`) renders a forward-looking
  7-day × 24-hour fire-density grid that aggregates the next week's schedule of every
  enabled cron job and routine into one color-coded matrix, so an operator can see
  fleet-wide busy windows, scheduling collisions, and open time slots at a glance.
  Three toggle buttons filter the grid to ALL / CRON / ROUTINES, and the current day
  and hour are highlighted. The grid auto-refreshes every 30 s and the "now" column
  advances every minute. Pure host-testable aggregation math; no backend change.
  Closes #625.
- **Live auto-refresh for the cron-jobs & routines tables.** Each list's action row
  gains a Grafana/Datadog-style refresh-interval selector (`Off` default, `5s`, `15s`,
  `30s`, `60s`) plus an "updated Ns ago" freshness cue, so an operator can keep the data
  current on a cadence they choose instead of reloading the SPA. The choice persists to
  `localStorage` under a shared key, so it is consistent across both pages and survives
  navigation and reload; `Off` preserves the historical load-once behaviour (no background
  traffic until opted in). Re-fetches use the existing `GET /api/v1/cron-jobs` /
  `GET /api/v1/routines` endpoints — no backend change. Closes #618.
- **Operations overview landing page.** The root `/` route now serves a single-pane
  OVERVIEW summary that aggregates both cron jobs and routines, so an operator sees
  whole-system state at a glance: five cross-entity KPI tiles (`SCHEDULED`, `ENABLED`,
  `DUE SOON`, `DISABLED`, `NEXT RUN` with a live countdown) and an UPCOMING RUNS table
  of the next 8 fires across every enabled job and routine, each tagged `CRON`/`ROUTINE`.
  Closes #606.
- **`NEXT RUN` column and `DUE SOON` KPI tile.** The scheduled-jobs table gains a
  `NEXT RUN` column showing the absolute next fire time plus a relative countdown
  (`in 5m`, `in 2h 10m`, `tomorrow 09:00`); disabled jobs read `paused` and the countdown
  turns red once a fire lands inside the due-soon window. A new `DUE SOON` KPI tile counts
  enabled jobs firing within the next hour, and a 30 s tick keeps countdowns live without
  a manual reload. Closes #597.
- **Faceted filter toolbar for the Routines page.** The single repository-URL
  substring filter is replaced with a multi-facet toolbar matching the Cron Jobs
  page (Airflow / GitHub Actions / Buildkite best practice: free-text + facets +
  live result count). New facets: full-text search across title, agent, schedule,
  schedule description, and repository URLs; status (All / Enabled / Disabled /
  Dormant); agent (Any / claude / codex / …); machine (Any / Unassigned / specific).
  A live "Showing N of M" count updates with each keystroke, a CLEAR button appears
  when any filter is active, and the empty state distinguishes "no routines yet"
  from "no matches — clear filters". Pure filter logic is extracted to free
  functions with 31 new host-side unit tests. Closes #642.

### Changed

- Enabled the `clippy::redundant_closure_for_method_calls` lint and fixed the
  violations, replacing closures that only forward their receiver to a method
  (`|e| e.ok()`, `|s| s.to_string()`, `|p| p.into_inner()`) with the method
  path itself (`Result::ok`, `ToString::to_string`,
  `std::sync::PoisonError::into_inner`). No behavior change.
- Pinned the `AppError` HTTP response **body** contract: tests now assert that
  every variant serializes to `{"error": <message>}`, not just the right status
  code, so the JSON error envelope clients parse can't silently regress. Tests
  only; no behavior change.
- Marked every public path builder in `paths` (`jobs_dir`, `routine_toml_path`,
  `pid_file`, `moadim_home`, …) `#[must_use]`. These functions are pure and the
  returned `PathBuf` is the whole point of calling them, so discarding it is
  always a mistake; the attribute lets clippy flag such a call at compile time
  instead of letting it silently no-op. No behavior change.

### Fixed

- The max-runtime watchdog now runs on its own 30s cadence instead of riding the
  hourly cleanup sweep, so a hung run is force-killed within ~30s of its
  `effective_max_runtime_secs` rather than surviving up to ~1h past its bound. A
  sub-hour `max_runtime_secs` (or a sub-hour cron interval) is now actually
  enforceable. TTL-reaping of finished workbenches stays on the hourly sweep.
  (#436)

- Removed a duplicate `.logo { font-weight }` declaration in `ui/index.html` left by
  the concurrent merge of #595 and #596; identical rendering, cleaner CSS. Closes #599.

### Fixed

- The generated routine launch script now anchors each workbench under
  `paths::workbenches_dir()` (which honours `MOADIM_HOME_OVERRIDE`) instead of
  hardcoding `$HOME/.moadim/workbenches`, so the launch path no longer drifts
  from the reaper and the LOGS view when the home override is set. Closes #601.

## [0.15.0] - 2026-06-21

### Added

- **Day calendar view.** Routines and cron jobs gain a scrollable single-day
  timeline: 24 hour rows with each fire time rendered as an `HH:MM` chip in its
  hour, prev/next/`TODAY` navigation, and the current hour highlighted and
  scrolled into view. Available alongside the routines `LIST`/`CALENDAR` toggle
  and as a new `LIST`/`DAY` toggle on the previously table-only cron-jobs page.
- **Zoom into the day view.** The single-day timeline gains a `−`/`+` zoom
  control with four per-hour heights. The compact level keeps the wrapped-chip
  layout; deeper levels switch each hour into a minute-positioned timeline where
  fire times float at their exact minute against quarter-hour guide lines and a
  `:00/:15/:30/:45` ruler, so sub-hour timing is readable at a glance. Closes #591.
- **Set machines from the web UI.** The routine and cron-job create/edit forms now
  expose a `MACHINES` input (comma-separated), so multi-machine targeting is settable
  without dropping to the CLI or REST. Blank preserves today's behavior (empty list =
  runs nowhere). Closes #580.
- **Machine picker.** The `MACHINES` field in the routine and cron-job forms is now a
  picker: it fetches the daemon's known machine names from the new `GET /api/v1/machines`
  endpoint (every name referenced by a routine or cron job, plus this machine's own
  identity) and renders them as toggleable chips, while still allowing a brand-new name
  to be typed and added. Closes #586.

### Changed

- **Releases are automated on version bump.** Merging a `Cargo.toml` version bump to
  `main` now auto-pushes the matching `vx.y.z` tag and runs the crates.io publish and
  GitHub Release workflows (new `auto-release.yml`). No more manual tag push; `publish.yml`
  and `release.yml` are now reusable (`workflow_call`) and keep their `v*` tag-push
  trigger as a hand-cut fallback.

### Fixed

- **Test isolation.** The routine service and storage unit tests no longer write
  into the developer's real `~/.config/moadim` home. They resolved paths through
  `paths::home()`, which falls back to the real home when `MOADIM_HOME_OVERRIDE`
  is unset, so tests leaked routine dirs (and the migration tests even scanned real
  state). Every test now runs against an isolated temp home.

## [0.14.0] - 2026-06-21

### Added

- **Multi-machine targeting.** Routines and cron jobs now carry a `machines` list,
  so one shared `~/.config/moadim` config repo can drive different routines/jobs on
  different machines (e.g. a laptop, a work box, a server). Each daemon resolves its
  own machine identity — `MOADIM_MACHINE` env, else the `name` in the gitignored
  `~/.config/moadim/machine.local.toml`, else the system hostname — and its crontab
  sync schedules only the entries naming that machine. A new `moadim machine`
  command (`show` / `set <name>` / `list`) inspects and sets the identity. The
  `machines` field is settable via REST, the MCP `create_*`/`update_*` tools, and
  the `--machines '["work","server"]'` CLI flag.
  **Note:** an empty `machines` list runs **nowhere** — an entry is dormant until
  assigned, so routines/jobs created before this change stop scheduling until you
  assign them (the daemon logs each unassigned entry once at sync time). The
  built-in default routine self-assigns to the machine that first seeds it.
- `moadim status --json` now folds the running server's `GET /health` details into
  its object as `uptime_secs` and `version`, so a single call answers liveness
  **and** age/version instead of forcing a second `curl /health`. Both fields are
  `null` when no server answers or its `/health` body cannot be parsed; exit codes
  and the human-readable `status` output are unchanged (#284).

### Changed

- Enabled the `clippy::map_unwrap_or` lint and fixed the violations, replacing
  `map(...).unwrap_or(...)` / `map(...).unwrap_or_else(...)` chains with the more
  direct `map_or` / `map_or_else`. No behavior change. (#524)
- Enabled the `clippy::semicolon_if_nothing_returned` lint and fixed the existing
  violations so statements that return `()` end with a trailing semicolon. No
  behavior change.
- Enabled the `clippy::manual_let_else` lint and rewrote the `match` guards
  whose only non-binding arm diverged (`return`/`continue`) as
  `let ... else { ... }`, keeping the happy path unindented. No behavior change.

### Fixed

- 6-field cron schedules (`sec min hour dom month dow`, accepted by `croner`)
  are now projected to a valid 5-field OS crontab line instead of being written
  verbatim. Previously only 7-field expressions had their leading seconds
  stripped, so a valid 6-field schedule reached the crontab unchanged — where
  vixie-cron/cronie either rejects the line (silently dropping every managed
  job) or misreads seconds as minutes (shifting the schedule). `normalize_schedule`
  and `to_os_schedule` now handle the 6-field form the same way as 7-field.
- The iCal feed (`GET /routines.ics`) no longer silently stops short of its
  advertised 30-day horizon for high-frequency routines. The per-routine
  `MAX_EVENTS_PER_ROUTINE = 100` cap still bounds feed size, but when a routine
  fires more often than the cap allows within the horizon, a trailing
  truncation-marker `VEVENT` (UID `…-truncated@moadim`) is now appended at the
  first omitted fire time, so calendar subscribers can see the projection was
  capped and where it stops instead of the routine appearing to just end after a
  few days (#251).
- Added a `MOADIM_TMUX_BIN` test seam to the cleanup sweep's tmux side-effects so tests never probe or kill sessions on the real tmux server; in test builds it falls back to a non-existent path. Mirrors the `MOADIM_CRONTAB_BIN` guard. (#215)- Routine iCal feed events are now `TRANSP:TRANSPARENT` instead of the default
  OPAQUE, so subscribing to the `.ics` feed no longer marks the operator BUSY at
  every scheduled fire time. A fire is a momentary trigger, not reserved time. (#461)
- Routine `update` now rejects a `ttl_secs` / `max_runtime_secs` that exceeds the
  cron-derived ceiling for the *effective* schedule (the new schedule if supplied,
  otherwise the routine's current one). The check runs before any mutation, so a
  rejected update leaves the in-memory store untouched. (#468)
- `launchctl_bin()` no longer falls back to the real `launchctl` in test builds.
  A `#[cfg(test)]` structural guard resolves the default to a nonexistent path
  (`/nonexistent/moadim-test-launchctl-guard`) so a macOS test that forgets to
  wire up the `MOADIM_LAUNCHCTL_BIN` shim cannot mutate the developer's live
  launchd session; the eventual spawn fails harmlessly. Mirrors the `crontab_bin()`
  guard from #211 (#213).
- The OpenAPI `servers` URL is now host-relative (`/api/v1`) instead of a
  hardcoded `http://127.0.0.1:5784/api/v1`. Swagger UI's "Try it out" now targets
  the origin the docs were served from, so it follows a custom `MOADIM_BIND_ADDR`
  port or a reverse proxy instead of failing against an address the daemon may not
  be bound to. (#385)
- An agent config that exists on disk but cannot be read (due to a permissions
  error or path collision) is now reported as `AgentLoadError::Unreadable` rather
  than `AgentLoadError::Missing`. Previously, any `read_to_string` failure was
  silently mapped to `Missing`, causing `validate_agent` to accept the broken
  config (it tolerates `Missing` for configs not yet created), leaving a
  green-dot routine that silently never fires. The new `Unreadable` variant is
  rejected at create/update time with a `BadRequest`, so the operator learns the
  real cause immediately. (#445)
- The routine-origin disclosure write into the workbench `CLAUDE.md` now
  fail-fasts. Previously this `printf > "$WB/CLAUDE.md"` was `;`-joined with no
  failure guard, so if the write failed (read-only/full `$HOME`, an unwritable
  `$WB`, disk-quota/inode exhaustion) the launch fell through to `cp prompt.md`,
  setup, and `tmux new-session`, starting the Claude agent with no `CLAUDE.md` —
  hence no routine-origin disclosure mandate. It now aborts the launch (logging
  to `agent.log` and stderr) exactly like the adjacent `cp prompt.md` guard. The
  optional user-prompt append remains best-effort (#482).

## [0.13.0] - 2026-06-21

### Added

- **Full action parity across the CLI, REST, and MCP surfaces.** Every cron-job
  and routine action is now reachable from all three.
  - **New CLI data commands** (thin clients over the running server's REST API,
    built on `clap`): `moadim cron-jobs <create|list|get|update|replace|delete|trigger|logs>`,
    `moadim routines <create|list|get|update|replace|delete|trigger|logs|ical>`,
    `moadim agents`, and `moadim echo <message>`. They print the server's JSON
    response and exit `3` ("not running") when no daemon is reachable, matching
    the existing `status`/`stop`/`cleanup` contract. (`cron`/`routine` are
    accepted as aliases.)
  - **New MCP tools** filling the gaps versus REST: `list_agents`,
    `cron_job_logs`, `routine_logs`, `shutdown`, and `restart`.
- **New `moadim schedule trigger <id>` CLI command** and matching
  `POST /api/v1/routines/{id}/scheduled-trigger` route. Runs a routine on its
  schedule, recording a *scheduled* (not manual) trigger. The generated crontab
  line invokes it directly at each fire time.
  - **New `POST /api/v1/restart` route** (plus the matching `restart` MCP tool):
    stops the running server and starts a fresh instance via a detached helper
    process, since an in-process server cannot rebind its own port. Documented in
    the OpenAPI spec.
- The MCP `health` tool now reports build provenance — `version`, `git_sha`, and
  `build_date` — bringing it to parity with `GET /api/v1/health` and
  `moadim --version`, so an MCP client can tell exactly which build is running
  rather than only seeing status, uptime, and filesystem locations (#476).
- The binary now embeds the git commit it was built from, so you can tell
  exactly which build is running rather than only the released crate version
  (which changes only on a `v*` tag). `moadim --version` prints
  `moadim <version> (<short-sha> <date>)`, and the `GET /api/v1/health` response
  gained `git_sha` and `build_date` fields alongside `version`. `build.rs`
  resolves the fields from git at compile time and falls back to `"unknown"`
  when the source isn't a git checkout (e.g. a crates.io tarball), so published
  builds still compile and report sensibly (#367).
- Routines now track **`last_scheduled_trigger_at`** (Unix seconds), the mirror of
  `last_manual_trigger_at` for scheduled cron firings, surfaced in the REST/OpenAPI
  routine response. Because the OS crontab runs a routine's generated `run.sh`
  directly — the daemon never observes a scheduled fire — the script itself stamps
  the fire time into a new gitignored `scheduled.local.toml` sidecar, which the
  daemon reads back on load. The sidecar is daemon-read-only and kept separate from
  the manual-trigger `state.local.toml`, so re-persisting a routine can't clobber a
  scheduler-written timestamp. This makes scheduled vs. manual runs distinguishable
  and lets you spot schedules that have never actually fired (#155).
- `moadim stop` accepts a `--quiet`/`-q` flag that suppresses the human-readable
  status line (`moadim is shutting down` / `moadim is not running`) while keeping
  the exit-code contract (`0` when a server was stopped, `3` when none was
  running), so scripts that branch on `$?` alone get no stdout noise. The flag is
  ignored under `--json`, which always prints its single machine-readable object.
- `moadim stop --json` now includes the bound `address` field
  (`{"running":bool,"pid":N|null,"address":"127.0.0.1:5784"}`), matching
  `status --json`'s object shape exactly so both can be parsed uniformly.
- `moadim cleanup --json` now includes the bound `address` field
  (`{"running":bool,"removed":N,"address":"127.0.0.1:5784"}`), matching
  `status --json`/`stop --json` so every `--json` command surfaces the endpoint
  it talked to, not just the running-state and result.
- The web UI header now shows the running daemon version (e.g. `/ v0.12.0`)
  next to the `MOADIM / CONTROL` logo. The `GET /api/v1/health` response gained
  a `version` field (from `CARGO_PKG_VERSION`) that the UI already-polled health
  request surfaces, so no extra request is made.
- Routine create/update now validates and normalizes `repositories` entries:
  blank or whitespace-only `repository` values (and blank `branch` values when
  set) are rejected with a `400 Bad Request` instead of being silently
  persisted, and surviving entries are trimmed. Malformed `repositories` lists
  are now caught at the API boundary rather than surfacing later as a confusing
  run-time failure (#241).
- Defense-in-depth security response headers are now injected on every HTTP
  response served by the daemon (web UI + `/api/v1`): `X-Frame-Options: DENY`
  and a `frame-ancestors 'none'` CSP block clickjacking of the dashboard's
  destructive controls, `X-Content-Type-Options: nosniff` stops content
  sniffing, and `Referrer-Policy: no-referrer` keeps the loopback URL from
  leaking to third parties. The CSP is intentionally scoped to `frame-ancestors`
  only so the existing inline + WASM SPA and Swagger UI keep working untouched
  (#406).

### Changed

- `moadim --help` now documents every flag the parser accepts, including the
  `-f`/`--foreground` and `-d`/`--detach`/`--daemon` aliases and the `--version`
  long form, so the help text can no longer silently drift from what `moadim`
  actually parses; a new test asserts every accepted flag appears in the help
  text (#340).
- HTTP request logs now carry a short per-request correlation id. Each request
  emits an inbound line (`[0000001a] <- GET /api/v1/health`) and an outbound
  line (`[0000001a] -> 200 /api/v1/health in 2ms`) sharing the same id, so the
  two halves can be paired in the log even when requests interleave under
  concurrency (previously the unprefixed `  -> …` line couldn't be matched to
  its request) (#354).
- Renamed the misleading `last_triggered_at` field to **`last_manual_trigger_at`**
  on both routines and cron jobs (TOML, REST/OpenAPI, MCP tool descriptions, and
  the web UI). The field was only ever updated by *manual* triggers, never by
  scheduled cron firings, so the old name wrongly read as "never ran" for a
  routine that fires on schedule but was never triggered by hand. Deserialization
  accepts the legacy `last_triggered_at` key via a serde alias, so existing
  `routine.toml` / job files still load.
- Service tests no longer touch the real user crontab; they run against an
  isolated test crontab seam.
- moadim-generated `.gitignore` files (job and routine) now ignore
  user-specific `run.sh` scripts.
- The config tree now honors `$XDG_CONFIG_HOME` per the XDG Base Directory
  spec: when set to an absolute path, the config dir resolves to
  `$XDG_CONFIG_HOME/moadim` instead of always `~/.config/moadim` (an unset,
  empty, or relative value still falls back to `~/.config`). This brings the
  config tree in line with the Linux systemd installer, which already resolved
  its unit path via `$XDG_CONFIG_HOME`, so users who relocate their config root
  no longer get a surprise second tree under `~/.config`.
- Routines no longer generate a per-routine `run.sh` launch script. The crontab
  line now invokes the `moadim` binary directly
  (`<schedule> <moadim> schedule trigger <id>`), and the running daemon is the
  single source of truth for launch logic — eliminating the duplication between
  the cron path and the manual-trigger path. Stale `run.sh` files left by older
  daemons are removed on the next persist. **Scheduled routines now require the
  daemon to be running** (it is installed as an OS service for this reason); the
  agent still inherits the user's login environment via the daemon's `sh -lc`
  spawn.
- Enabled the `clippy::uninlined_format_args` lint (deny) and inlined the
  existing positional format arguments (`"{}", x` → `"{x}"`) so log lines and
  error messages read more directly. No behavior change.

### Removed

- Removed the vestigial `echo` demo endpoint/tool — the scaffold `POST
  /api/v1/echo` REST route, the `echo` MCP tool, and their `EchoRequest` /
  `EchoResponse` / `EchoInput` types and OpenAPI entries. It echoed a message
  back with a server timestamp, served no product purpose, and only widened the
  REST + MCP + OpenAPI surface; `GET /health` already covers liveness probing.
  The committed `apis/openapi.json` is regenerated without the `/echo` path and
  schemas (#359).

### Fixed

- Crontab block replacement now matches its delimiters as whole lines instead of
  raw substrings, guarding against a marker prefix-matching a more specific one
  elsewhere in the crontab and silently overwriting it. (#324)
- An unknown or mistyped command (e.g. `moadim staus`) is no longer treated as a
  success. The parser now classifies an unrecognized first argument as a usage
  error distinct from an explicit `help`/`-h`/`--help` request: it prints
  `unknown command: <arg>` plus a hint to **stderr** and exits **2**, instead of
  printing help to stdout and exiting `0`. Explicit `help` is unchanged (stdout,
  exit `0`). This keeps the script-friendly exit-code contract intact so a
  wrapper, systemd unit, or CI step can detect a typo. (#303)
- `moadim stop` / `POST /api/v1/shutdown` no longer hangs forever when a
  long-lived connection stays open. Axum's graceful shutdown waits for every
  in-flight connection to close, so an open `/mcp` SSE stream (or any slow
  client) could keep the serving loop pending indefinitely. The server now
  bounds the post-shutdown drain to a grace window (default 10s, overridable via
  `MOADIM_SHUTDOWN_GRACE_MS`): connections still draining when it elapses are
  abandoned and the process exits cleanly, logging a warning. (#342)
- Repaired eleven broken `rustdoc` intra-doc links so `cargo doc` builds clean
  again. The crate root's `#![deny(warnings)]` implies
  `deny(rustdoc::broken_intra_doc_links)`, but nothing ran `cargo doc` in CI or
  the pre-push hook, so the rotted links sat on `main` and made `cargo doc` fail
  with "could not document `moadim`". Links to private submodules in
  `src/routines/mod.rs` were demoted to plain code spans, and the remaining
  links in `cleanup`, `sync`, and `utils::atomic` were fully qualified. (#390)
- The in-memory routine and cron-job stores no longer panic the request that
  observes a poisoned lock. Every `Mutex::lock().unwrap()` on these stores was
  replaced with a new `LockRecover::lock_recover()` extension that recovers the
  guard from `PoisonError` (the protected `HashMap` is still structurally valid),
  so one panicking handler can't cascade into every later request taking the same
  lock. The two `get_mut(id).unwrap()` invariant unwraps in `svc_update`/
  `svc_trigger` became `ok_or(AppError::NotFound)?`, removing the last panicking
  unwraps from the production code paths. A new
  `#![cfg_attr(not(test), deny(clippy::unwrap_used))]` crate lint now keeps
  `.unwrap()` out of non-test code so the panic can't creep back in (tests still
  use `.unwrap()` freely, where panicking is the intended failure mode).
- Managed cron jobs are now re-synced to the OS crontab on daemon startup,
  mirroring the routines sync that already ran. Previously the cron-job block was
  only written on a job create/update/delete, so if it was lost or emptied
  (manual `crontab -e`/`crontab -r`, an OS migration, or a marker collision) every
  managed job stayed silently un-fired until the next mutation — even across a
  restart, while routines self-healed. The startup sync is idempotent, so it is a
  no-op read on a healthy crontab. (#394)
- The generated `prompt.md` no longer emits a dangling "These repositories are
  relevant — clone any you need:" header with an empty bullet list when a routine
  has no `repositories`. `compose_prompt` now writes a plain "You are working in
  an empty directory." preamble in that case, so the agent isn't promised a repo
  list with nothing under it.
- Deflaked `stop_running_and_wait_force_kills_then_succeeds_when_server_goes_down`:
  the test raced a ~35ms window between the restart timeout (80ms) and the server
  drop (130ms), so a coverage-instrumented or loaded CI run could miss the post-kill
  `wait_until_stopped` window and fail the assertion. The margins are now 300ms /
  450ms, giving ~150ms of slack on each side of the deadline while still exercising
  the same force-kill-then-stops path.
- A malformed (present-but-unparseable) agent TOML is no longer misreported as
  "agent config not found". `load_agent_command` now returns a `Result` with a
  distinct `Missing` vs. `Parse` failure, so the sync/trigger skip diagnostics
  name the agent and quote the underlying `toml` parse error. Creating or
  updating a routine that references a malformed agent config is now rejected
  with `400 Bad Request` (REST + MCP) at edit time instead of being silently
  skipped at fire time. The missing-file case is unchanged (still skipped and
  warned, with an accurate message). (#189)
- Unknown paths under `/api/v1` now return a JSON **404** instead of the SPA
  `index.html` with `200`. The nested API router had no fallback of its own, so
  in axum 0.8 it inherited the outer SPA `.fallback(get(index))` — a typo'd or
  removed endpoint answered with HTML/200, surfacing as a confusing downstream
  parse error rather than a clear not-found. The API router now owns a JSON 404
  fallback while the SPA fallback still serves UI routes (#270).
- Crontab docs no longer claim reverse sync (crontab → moadim) runs. It is
  implemented but never wired to a poller or startup hook, so manual edits to
  the moadim block do not round-trip and are overwritten by the next forward
  sync. The in-crontab header, README "Crontab sync" section, and module/`main`
  docs now say so instead of promising automatic sync-back (#218).
- `uptime_secs` is now clamped against backward clock skew (saturating
  subtraction) so it never underflows.
- Routine create/update now validates the configured agent, rejecting unknown agents.
- The daemon now installs a logging backend at startup so `log` calls
  actually emit output instead of being silently dropped.
- `moadim status` now reports the effective bind address instead of the
  hardcoded default when a custom bind address is configured.
- `moadim stop --json` now reports the effective bind address (`address`
  field) instead of the hardcoded default, matching `status --json`. It hardcoded
  `BIND_ADDR` while `status --json` already honored the bind override, so the two
  shapes drifted apart whenever a custom bind address was configured.
- iCal `escape_text` now normalizes carriage returns (CR and CRLF) to `\n`
  per RFC 5545, so generated calendar feeds no longer emit raw control
  characters in escaped text.
- Cron `@keyword` documentation now matches the actual validation contract,
  aligning the documented and accepted set of `@`-keywords.
- Routine create/update now reject nonsensical field values with `400 Bad
  Request` instead of silently persisting a broken routine. A blank
  (empty/whitespace-only) `title` previously produced an empty routine-origin
  disclosure name and a bare `"routine"` slug (#226); a blank `prompt` made the
  routine fire forever with no task (#224); and a zero `ttl_secs` /
  `max_runtime_secs` instantly reaped the run's logs or self-killed the session
  (#233). All four are validated up front on both `POST` (create) and `PATCH`
  (update), before anything is written to disk or the crontab.
- Routine **create/update now reject a blank or unusable `title`** with
  `400 Bad Request`. A title must contain at least one alphanumeric character
  (so empty, whitespace-only, and punctuation-only titles like `"!!!"` are
  refused) and is capped at 200 characters. Previously such a title was accepted,
  producing a nameless routine-origin disclosure (`Routine name:` with nothing
  after it) in the workbench `CLAUDE.md` and a silent `"routine"` slug the user
  never chose.
- Route the macOS LaunchAgent `plist_path()` through the `MOADIM_HOME_OVERRIDE` home seam so service install/uninstall tests can no longer write to or delete the developer's real `~/Library/LaunchAgents/io.moadim.daemon.plist` (#214).
- `kill_pid` (the force-kill fallback in the restart path) now resolves its
  executable through an opt-in `MOADIM_KILL_BIN` seam, letting tests inject a
  harmless shim instead of signalling a real PID. The default stays the platform
  killer (`kill` / `taskkill`), so the existing self-contained test that kills
  its own spawned child still works. (#216)
- The `ui` crate's `RAction::Upsert` variant now boxes its `Routine`
  (`Upsert(Box<Routine>)`). The variant carried a ~272-byte `Routine` by value
  while the next-largest variant was 48 bytes, tripping
  `clippy::large_enum_variant` under the crate's `[lints.clippy] all = "deny"`,
  so `cargo clippy --all-targets` failed to compile. The reducer derefs the box
  once before the existing upsert logic, and the construction sites wrap the
  value.

### Fixed

- Routine **create/update now reject an empty or whitespace-only `prompt`** with
  `400 Bad Request` (`prompt must not be empty`), across the REST and MCP
  surfaces. Previously a blank prompt was accepted and synced to the crontab, so
  the routine fired on every tick and launched an agent with no task — silently
  burning scheduled runs and agent/API budget.

## [0.12.0] - 2026-06-18

### Added

- Per-routine **max-runtime watchdog** bounds hung agent runs. Routines carry an
  optional `max_runtime_secs` (TOML + REST/MCP create/update). Like `ttl_secs`,
  the effective bound is `min(MAX_RUNTIME_SECS, cron interval)` (default cap 1h),
  lowered further by an explicit `max_runtime_secs`. The hourly cleanup sweep now
  force-kills any tmux session
  whose run has exceeded its effective max runtime — recording
  `moadim: routine exceeded max runtime; killing session` in the run's
  `agent.log` — after which the workbench is reaped under the existing
  `ttl_secs` rules. A session still within its max runtime is never touched.
  Previously a hung run (waiting on stdin, looping, blocked on a stuck
  network/git op) lived forever and stacked one zombie session + workbench per
  cron tick, since the TTL reaper only governs *finished* runs.
- `moadim install` / `moadim uninstall` register the daemon as an OS service so
  it starts at login and is restarted on crash, keeping scheduled routines firing
  across reboots. macOS writes a per-user launchd LaunchAgent
  (`~/Library/LaunchAgents/io.moadim.daemon.plist`, loaded with `launchctl`);
  Linux writes a systemd **user** service (`~/.config/systemd/user/moadim.service`,
  enabled with `systemctl --user enable --now`). Both run the daemon in the
  foreground (`moadim --interactive`) so the service manager supervises it; other
  platforms report that the command is not yet supported.
- **Hermes** is now a built-in agent alongside `claude` and `codex`. A default
  `hermes.toml` (`hermes exec {prompt_file}`, mirroring Codex) is seeded into
  `~/.config/moadim/agents/` on startup, and `hermes` appears in
  `available_agents()` / `GET /agents`, so routines can launch Hermes.

### Changed

- Routine runtime state (last-run timestamps and related mutable fields) is now
  stored in a separate, git-ignored sidecar file instead of the git-tracked
  `routine.toml`, so scheduled runs no longer produce noisy diffs or merge
  conflicts in version-controlled routine definitions (#127).

### Fixed

- iCal feeds now fold long content lines at 75 octets per RFC 5545 §3.1, using a
  UTF-8-aware byte budget so multi-byte characters are never split across a fold
  boundary. Previously over-long `SUMMARY`/`DESCRIPTION` lines were emitted
  unfolded, which some calendar clients reject.
- `now_secs()` no longer panics when the system clock reads before the Unix
  epoch (1970). A VM or container booted with a dead real-time clock could make
  `SystemTime::duration_since` fail and crash the daemon; such readings are now
  clamped to `0` until the clock is corrected.
- Several `svc_*` routine-service tests no longer overwrite the developer's real
  user crontab. `svc_create`/`svc_update`/`svc_delete` sync the crontab, and four
  tests exercised them without isolating the `crontab` binary, so running the
  suite locally replaced the live routines block with a single test fixture line.
  The tests now run under an empty `PATH` so the sync cannot spawn `crontab`
  (#175).
- The crontab binary resolver now refuses to fall back to the real system
  `crontab` in test builds when no `MOADIM_CRONTAB_BIN` shim is configured,
  returning a non-existent path so the spawn fails harmlessly. This is a
  structural safety net: no test — current or future — can clobber the
  developer's live crontab even if it forgets to isolate the binary (#175).

## [0.11.2] - 2026-06-17

### Fixed

- Scheduled routine agents now run under a **login shell** (`/bin/sh -l '<run.sh>'`
  in the crontab line; `sh -lc` for manual triggers), so the agent sources the
  user's `~/.profile` and inherits their environment variables — `GH_TOKEN`, API
  keys, etc. Previously routines launched with cron's minimal environment and,
  on macOS, outside the GUI login session, so tools like `gh`/`git` had no
  credentials and could not authenticate. `PATH` is still replaced with the same
  curated list as before, so binary resolution is unchanged — only environment
  variables are gained. Put any environment the agent needs (e.g. `export
  GH_TOKEN=…`) in `~/.profile`.

## [0.11.1] - 2026-06-17

### Fixed

- Routine crontab sync no longer wipes the populated `MOADIM-ROUTINES` block
  when the routine store is empty. An empty store at sync time signals a load
  failure or a racing second daemon rather than a genuine "no routines" state
  (startup always reseeds the built-in defaults), and previously such a sync
  silently dropped every scheduled routine's cron line — leaving routines that
  never fired. The sync now detects this case and preserves the existing block.

## [0.11.0] - 2026-06-17

### Added

- The moadim-managed system prompt (`CLAUDE.md`) now carries a **routine-origin
  disclosure** section that instructs the agent to reveal, in every
  outward-facing communication (GitHub issues/PRs/comments, Slack, email, etc.),
  that the action originates from the running moadim routine — naming it. The
  routine name is injected at launch time. Internal logs and in-repo working
  files are exempt.
- Routine listings can now be filtered and sorted by repository. `GET /routines`
  accepts `repository` (case-insensitive URL substring filter), `sort`
  (`created`|`updated`|`title`|`repository`), and `order` (`asc`|`desc`) query
  parameters, and the Routines tab gains a filter/sort bar (repository input,
  sort dropdown, direction toggle). Defaults preserve the previous
  created-ascending behaviour.
- `moadim stop --json` now emits a single machine-readable object
  (`{"running":true}` when a running server was asked to shut down,
  `{"running":false}` when none was reachable), completing the `--json`
  contract alongside `status` and `cleanup`. The exit code is unchanged
  (`0` running, `3` not).

### Changed

- Restored 100% line coverage (enforced by the pre-push hook). To exercise the
  daemon-lifecycle, crontab-sync, and config-path code without touching the
  user's real environment, the binary gained test-only seams read from
  environment variables — `MOADIM_HOME_OVERRIDE` (config/routine/job paths),
  `MOADIM_BIND_ADDR` (server bind + client probe address),
  `MOADIM_CRONTAB_BIN` (the `crontab` executable), and
  `MOADIM_RESTART_TIMEOUT_MS`/`MOADIM_RESTART_POLL_MS` (restart stop-wait
  timing). They default to the previous behaviour when unset. The test harness
  is pinned single-threaded (`.cargo/config.toml`) so these overrides cannot
  race. No change to default runtime behaviour.

### Fixed

- Routine store writes are now atomic. `write_routine` persists `routine.toml`
  and `prompt.md` via a shared `atomic_write` helper (write a sibling temp file,
  then rename it into place) instead of an in-place `std::fs::write` truncate.
  A crash or full disk mid-write can no longer leave a torn `routine.toml` —
  which parsed to nothing and silently dropped the routine from the store — and
  the continuously-running reverse crontab sync never observes a partial file.
- Routine logs (`GET /routines/{id}/logs`) could return another routine's log
  when one routine's slug is a dash-prefix of another's (e.g. `logs` vs
  `logs-extra`): the newest-workbench lookup matched a bare `{slug}-` prefix,
  so `logs-extra-<ts>` was wrongly attributed to `logs`. It now matches the
  slug exactly via the same `{slug}-{ts}` parser the cleanup sweep uses, and
  picks the newest run by numeric timestamp instead of a lexicographic compare
  over the directory name.
- Restored `cargo clippy` compliance across the crate. The `min_ident_chars`
  and `missing_docs` lints (both `deny` in `Cargo.toml`) were failing on
  current stable, which also broke the pre-push hook. Renamed all single-letter
  bindings to descriptive names and documented the remaining undocumented
  fields — no behavioral change.

### Documentation

- Added a **Scripting** table to the README that documents the `--json` object
  shapes for `status` (`{"running":bool,"pid":N|null,"address":…}`), `cleanup`
  (`{"running":bool,"removed":N}`), and `stop` (`{"running":bool}`) alongside
  their exit codes, so the machine-readable contract is discoverable without
  reading `--help`. Also documents `moadim stop --json`, which was previously
  only mentioned in `--help`.

## [0.10.0] - 2026-06-17

### Added

- Built-in **default routines**, seeded and kept current on startup. The first
  ships out of the box: _Update moadim cargo package_, which runs daily at 09:00
  local time and tasks the agent with checking the installed `moadim` cargo
  package against the latest crates.io release and running
  `cargo install moadim --force` when it is behind. Defaults are daemon-owned —
  schedule, agent, and prompt are refreshed from the built-in spec on every
  start so improvements ship on upgrade — but the user's `enabled` toggle is
  never overridden: a default the user turns off stays off across restarts.

## [0.9.0] - 2026-06-17

### Fixed

- Routines created before the UUID→slug storage-directory change launched their
  agent with an empty prompt: their `routine.toml`/`prompt.md` stayed in the
  legacy `{id}/` dir while the crontab sync wrote `run.sh` into a fresh `{slug}/`
  dir, so the cron `cp prompt.md` read an empty dir. Startup now migrates legacy
  UUID-named routine dirs to the slug layout and re-persists every routine's
  `routine.toml` + `prompt.md` sidecar, healing dirs left with only `run.sh`. The
  generated `run.sh` also now aborts (and logs to the workbench `agent.log`) when
  the source prompt is missing instead of launching a task-less agent session.

### Added

- `moadim restart` CLI subcommand that stops a running daemon (if any) and
  starts a fresh detached background instance.
- `moadim restart` now prints the PID rotation as `restarted: pid <old> -> <new>`
  (old reads `none` when nothing was running) so scripts/logs can confirm the
  process was actually replaced.
- Script-friendly exit-code contract for `moadim status` and `moadim cleanup`:
  they exit `0` when a server is running and `3` when none is, so callers can
  branch on `$?` without parsing stdout.
- Multiselect in the web UI cron-jobs table: select rows via click /
  `Shift`+click range / `Cmd`/`Ctrl`+click toggle, a select-all checkbox, and a
  bulk-action bar to enable, disable, or delete the selected jobs at once.

## [0.8.0] - 2026-06-16

### Added

- This changelog.
- `GET /routines.ics` iCalendar feed of upcoming routine fire times for
  subscribing in external calendars.
- `--json` flag for `moadim status` and `moadim cleanup` so their output can be
  consumed by scripts.
- CI `Changelog` workflow that fails a PR touching `src/` or `ui/` when
  CHANGELOG.md is not updated, bypassable with a `skip-changelog` label.

### Changed

- Split the UI into separate cron jobs and routines pages, and moved the REST
  API under the `/api/v1` prefix.

### Fixed

- Restore the build under `#![deny(warnings)]` and regenerate the committed
  OpenAPI spec, both left stale by the cleanup-module/TTL refactor.

## [0.7.0] - 2026-06-16

### Added

- Auto-cleanup of finished routine run workbenches with a per-routine TTL.

### Changed

- Surface the routine schedule timezone in MCP tools and responses.

### Fixed

- Apply the UI size-optimization release profile at the workspace root so the
  server binary keeps its default release profile.
- Move the OpenAPI inline test into a sibling `_tests.rs` file to satisfy the
  test-file convention.

## [0.6.1] - 2026-06-15

### Changed

- Document that cron schedules use the host local timezone, not UTC, across the
  README, Architecture guide, and TODO list.

## [0.6.0] - 2026-06-15

### Added

- Validation dialog before shutdown (groundwork in TODOs).
- Per-routine TTL for triggered routines to prevent indefinite retention.
- Write a `.gitignore` for generated runtime files in the config directory.

### Changed

- Use the slugified routine title as the run folder name instead of the UUID.
- Rename the prompt sidecar from `prompt.txt` to `prompt.md`.

### Fixed

- Remove the unused `MOADIM_BUILD_UI` variable from the build script.

## [0.5.0] - 2026-06-15

### Fixed

- Atomic, locked write of `~/.claude.json` during Claude agent setup.
- Correct a typo in the TODOs description.

## [0.4.0] - 2026-06-15

### Added

- `GET /agents` endpoint and an agent dropdown in the routines UI.

### Fixed

- Make the cron-job edit/delete modals clickable in the UI.

## [0.3.0] - 2026-06-15

### Added

- Run the server in background or interactive mode, killable from the client.

## [0.2.0] - 2026-06-15

### Added

- Routines: agent-driven scheduled jobs, with a dedicated routines tab in the UI.
- Logs page in the UI and `GET /cron-jobs/{id}/logs`.
- Swagger UI served at `/docs`.
- `schedule_description` field on `CronJobResponse` via croner.
- `CronJobSourceType` enum distinguishing managed vs OS jobs.
- 100% line coverage, enforced via the pre-push hook.

### Fixed

- Run routines via a generated `run.sh` so crontab lines stay under cron's
  length limit.
- Make scheduled routines actually fire from cron.
- Execute the handler script when the trigger endpoint is called.

## [0.1.0] - 2026-06-11

### Added

- Persist cron jobs to `~/.config/moadim/jobs/`.
- Manual trigger for cron jobs via HTTP, MCP, and the UI.
- Type-safe Yew UI with the WASM bundle inlined at build time.
- Expose filesystem locations in response headers.
- Unify REST/MCP behind a shared service layer; include the job file path in
  responses.
- Extract storage and path-builder logic into dedicated modules.

### Fixed

- Ship the prebuilt UI in the published crate.
- Rename the binary to `moadim` and add install docs.

[Unreleased]: https://github.com/moadim-io/daemon/compare/v1.6.1...HEAD
[1.6.1]: https://github.com/moadim-io/daemon/compare/v1.6.0...v1.6.1
[1.6.0]: https://github.com/moadim-io/daemon/compare/v1.5.0...v1.6.0
[1.5.0]: https://github.com/moadim-io/daemon/compare/v1.4.1...v1.5.0
[1.4.1]: https://github.com/moadim-io/daemon/compare/v1.4.0...v1.4.1
[1.4.0]: https://github.com/moadim-io/daemon/compare/v1.3.1...v1.4.0
[1.3.1]: https://github.com/moadim-io/daemon/compare/v1.3.0...v1.3.1
[1.3.0]: https://github.com/moadim-io/daemon/compare/v1.2.0...v1.3.0
[1.2.0]: https://github.com/moadim-io/daemon/compare/v1.1.0...v1.2.0
[1.1.0]: https://github.com/moadim-io/daemon/compare/v1.0.1...v1.1.0
[1.0.1]: https://github.com/moadim-io/daemon/compare/v1.0.0...v1.0.1
[1.0.0]: https://github.com/moadim-io/daemon/compare/v0.27.0...v1.0.0
[0.27.0]: https://github.com/moadim-io/daemon/compare/v0.26.0...v0.27.0
[0.26.0]: https://github.com/moadim-io/daemon/compare/v0.25.0...v0.26.0
[0.25.0]: https://github.com/moadim-io/daemon/compare/v0.24.0...v0.25.0
[0.24.0]: https://github.com/moadim-io/daemon/compare/v0.23.0...v0.24.0
[0.23.0]: https://github.com/moadim-io/daemon/compare/v0.22.1...v0.23.0
[0.22.1]: https://github.com/moadim-io/daemon/compare/v0.22.0...v0.22.1
[0.22.0]: https://github.com/moadim-io/daemon/compare/v0.21.0...v0.22.0
[0.21.0]: https://github.com/moadim-io/daemon/compare/v0.20.0...v0.21.0
[0.20.0]: https://github.com/moadim-io/daemon/compare/v0.19.1...v0.20.0
[0.19.1]: https://github.com/moadim-io/daemon/compare/v0.19.0...v0.19.1
[0.19.0]: https://github.com/moadim-io/daemon/compare/v0.18.0...v0.19.0
[0.18.0]: https://github.com/moadim-io/daemon/compare/v0.17.1...v0.18.0
[0.17.1]: https://github.com/moadim-io/daemon/compare/v0.17.0...v0.17.1
[0.17.0]: https://github.com/moadim-io/daemon/compare/v0.16.0...v0.17.0
[0.16.0]: https://github.com/moadim-io/daemon/compare/v0.15.0...v0.16.0
[0.15.0]: https://github.com/moadim-io/daemon/compare/v0.14.0...v0.15.0
[0.14.0]: https://github.com/moadim-io/daemon/compare/v0.13.0...v0.14.0
[0.13.0]: https://github.com/moadim-io/daemon/compare/v0.12.0...v0.13.0
[0.12.0]: https://github.com/moadim-io/daemon/compare/v0.11.2...v0.12.0
[0.11.2]: https://github.com/moadim-io/daemon/compare/v0.11.1...v0.11.2
[0.11.1]: https://github.com/moadim-io/daemon/compare/v0.11.0...v0.11.1
[0.11.0]: https://github.com/moadim-io/daemon/compare/v0.10.0...v0.11.0
[0.10.0]: https://github.com/moadim-io/daemon/compare/v0.9.0...v0.10.0
[0.9.0]: https://github.com/moadim-io/daemon/compare/v0.8.0...v0.9.0
[0.8.0]: https://github.com/moadim-io/daemon/compare/v0.7.0...v0.8.0
[0.7.0]: https://github.com/moadim-io/daemon/compare/v0.6.1...v0.7.0
[0.6.1]: https://github.com/moadim-io/daemon/compare/v0.6.0...v0.6.1
[0.6.0]: https://github.com/moadim-io/daemon/compare/v0.5.0...v0.6.0
[0.5.0]: https://github.com/moadim-io/daemon/compare/v0.4.0...v0.5.0
[0.4.0]: https://github.com/moadim-io/daemon/compare/v0.3.0...v0.4.0
[0.3.0]: https://github.com/moadim-io/daemon/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/moadim-io/daemon/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/moadim-io/daemon/releases/tag/v0.1.0
