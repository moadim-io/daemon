# Changelog

> _Today's TODO, tomorrow's changelog. Ship one, dream up two._

All notable changes to this project are documented here.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

Versions map to the `v*` git tags that drive the crates.io publish workflow.

## [Unreleased]

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

[Unreleased]: https://github.com/moadim-io/daemon/compare/v0.23.0...HEAD
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
