# Changelog

> _Today's TODO, tomorrow's changelog. Ship one, dream up two._

All notable changes to this project are documented here.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

Versions map to the `v*` git tags that drive the crates.io publish workflow.

## [Unreleased]

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

[Unreleased]: https://github.com/moadim-io/daemon/compare/v0.11.0...HEAD
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
