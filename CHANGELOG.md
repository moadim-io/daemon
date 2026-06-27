# Changelog

> _Today's TODO, tomorrow's changelog. Ship one, dream up two._

All notable changes to this project are documented here.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

Versions map to the `v*` git tags that drive the crates.io publish workflow.

## [Unreleased]

### Added

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

- Enabled the `clippy::map_unwrap_or` lint and fixed the violations
  (`map(...).unwrap_or(...)` → `map_or(...)`). No behavior change.

### Fixed
- **6-field cron expressions no longer silently fail to fire.** `croner` accepts
  6-field (`sec min hour dom month dow`) and 7-field expressions, but the OS
  crontab only understands 5 fields. Both forms are now normalised to 5 fields
  by dropping the leading seconds (and, for 7-field, the trailing year) before
  the expression is stored or written to the crontab. Previously a 6-field
  string was written verbatim, making the job malformed and silently inactive.
  Closes #183.

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
- Added a `MOADIM_TMUX_BIN` test seam to the cleanup sweep's tmux side-effects so tests never probe or kill sessions on the real tmux server; in test builds it falls back to a non-existent path. Mirrors the `MOADIM_CRONTAB_BIN` guard. (#215)
- Routine iCal feed events are now `TRANSP:TRANSPARENT` instead of the default
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

### Fixed

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

[Unreleased]: https://github.com/moadim-io/daemon/compare/v0.15.0...HEAD
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
