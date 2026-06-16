# Changelog

> _Today's TODO, tomorrow's changelog. Ship one, dream up two._

All notable changes to this project are documented here.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

Versions map to the `v*` git tags that drive the crates.io publish workflow.

## [Unreleased]

### Added

- `moadim restart` CLI subcommand that stops a running daemon (if any) and
  starts a fresh detached background instance.
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

[Unreleased]: https://github.com/moadim-io/daemon/compare/v0.8.0...HEAD
[0.8.0]: https://github.com/moadim-io/daemon/compare/v0.7.0...v0.8.0
[0.7.0]: https://github.com/moadim-io/daemon/compare/v0.6.1...v0.7.0
[0.6.1]: https://github.com/moadim-io/daemon/compare/v0.6.0...v0.6.1
[0.6.0]: https://github.com/moadim-io/daemon/compare/v0.5.0...v0.6.0
[0.5.0]: https://github.com/moadim-io/daemon/compare/v0.4.0...v0.5.0
[0.4.0]: https://github.com/moadim-io/daemon/compare/v0.3.0...v0.4.0
[0.3.0]: https://github.com/moadim-io/daemon/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/moadim-io/daemon/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/moadim-io/daemon/releases/tag/v0.1.0
