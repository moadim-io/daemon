# Changelog

> _Today's TODO, tomorrow's changelog. Ship one, dream up two._

All notable changes to this project are documented here.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

Versions map to the `v*` git tags that drive the crates.io publish workflow.

## [Unreleased]

### Added

- A workspace `xtask` crate with a **`cargo xtask spellcheck`** task that installs
  `typos-cli` (if the `typos` binary is missing) and runs it over the repo, reusing
  the existing `typos.toml`. Contributors no longer need to know the crate is
  `typos-cli` while the binary is `typos`; the pre-commit hook hint and
  `CONTRIBUTING.md` now point at the wrapper. `xtask` is the home for future
  tooling wrappers (#483).

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

### Changed

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
- Enabled the `clippy::uninlined_format_args` lint (deny) and inlined the
  existing positional format arguments (`"{}", x` → `"{x}"`) so log lines and
  error messages read more directly. No behavior change.

### Fixed

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

[Unreleased]: https://github.com/moadim-io/daemon/compare/v0.13.0...HEAD
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
