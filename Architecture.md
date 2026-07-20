# Moadim Architecture

> _One port to rule them all. Three protocols, one source of truth._
>
> _No moadim in the loop when it counts — the OS does the heavy lifting._

Moadim is a Rust daemon that manages scheduled AI-agent routines and exposes them over two protocols simultaneously — REST and MCP — on a single port (`127.0.0.1:5784`). It also serves an embedded browser UI: a React/TypeScript app, inlined into the binary at build time.

---

## High-level picture

```
                ┌─────────────────────────────────────────┐
                │           Axum HTTP server :5784         │
                │                                          │
  Browser ──────┤  GET /            (inlined HTML+JS)     │
  curl/SDK ─────┤  REST /routines   (JSON)                │
  AI agent ─────┤  /mcp             (MCP streamable-HTTP) │
                │                                          │
                │  Shared AppState:                        │
                │    RoutineStore  Arc<Mutex<HashMap>>     │
                └──────────────┬──────────────────────────┘
                               │ read+write on every mutation
                               ▼
               ~/.config/moadim/routines/
               ├── <uuid>/routine.toml                  (tracked; [env] = non-secret vars)
               ├── <uuid>/routine.local.toml             (gitignored, optional; secret env overrides)
               ├── <uuid>/prompts/prompt.pure.md         (tracked)
               ├── <uuid>/prompts/prompt.compiled.local.md (gitignored)
               └── <uuid>/.gitignore                    (generated)
```

---

## Source layout

```
src/
├── main.rs              entry point — binds socket, loads store, starts server
├── cli/                 CLI parsing + background-process lifecycle (status/stop/cleanup/…)
│   ├── mod.rs               top-level parser + start/stop/status lifecycle
│   ├── query.rs             server-query commands (cleanup/trigger/status)
│   ├── system.rs            pidfile + detached-process helpers
│   └── restart.rs           the `restart` command + detached-spawn reporting
├── commands.rs          data-plane CLI subcommands that drive a running server over HTTP
├── commands_http.rs     JSON request-body helpers + loopback HTTP request/response cycle shared by commands.rs
├── build_info.rs        compile-time build provenance (crate version + git commit/date)
├── error.rs             AppError → HTTP status codes
├── global_lock.rs       lock sentinel that halts all routine scheduling/triggers
├── logging/mod.rs       logging backend init (env_logger text, or JSON via MOADIM_LOG_FORMAT=json)
├── openapi.rs           utoipa ApiDoc definition served at /docs/openapi.json
├── restart.rs           replaces an already-running daemon with a fresh process
├── routine_storage.rs   routine.toml + prompts/ (pure/compiled) persistence
│
├── routes/
│   ├── http.rs                  Axum router assembly, re-exports run_with_listener_until
│   ├── mcp.rs                   MoadimMcp — rmcp tool_router, composes each endpoint's mcp.rs
│   ├── http_listener.rs         listener bind + graceful shutdown + run_with_listener_until
│   ├── http_settings_routes.rs  machine identity + persistent user-prompt settings routes
│   ├── metrics.rs               GET /api/v1/metrics — Prometheus-format process/routine metrics
│   ├── CONTRIBUTING.md          when/how to give an endpoint its own <name>/ folder
│   └── <name>/                  one folder per endpoint with both a REST route and an MCP tool
│       ├── mod.rs                   wiring only: declares submodules, re-exports the public surface
│       ├── logic.rs                 response type(s) + the pure function that builds them (no framework code)
│       ├── http.rs                  thin Axum handler: extracts state, calls logic, wraps in Json
│       └── mcp.rs                   thin MCP tool: calls logic, wraps in the MCP result type
│           (e.g. health/, create_routine/, list_routines/, get_routine/, delete_routine/,
│            list_routine_runs/, list_agents/, get_lock_status/, cleanup_workbenches/,
│            restart/, shutdown/ — see src/routes/CONTRIBUTING.md. Not every endpoint is
│            split out yet; e.g. update_routine, MCP-only, is still inline in mcp.rs)
│
├── middlewares/
│   ├── host_validation.rs    guards against DNS-rebinding / cross-origin abuse of the loopback API
│   ├── logger.rs             request/response logger
│   ├── security_headers.rs   adds CSP and related response headers
│   └── timeout.rs            per-request deadline for the REST API (/api/v1 only)
│
├── filesystem/mod.rs    FsLocation — server working dir + exe dir (surfaced via GET /health and the MCP `health` tool)
├── paths/mod.rs         path builders for ~/.config/moadim/routines/
├── machine/mod.rs       machine identity resolution (env/file/hostname)
├── service/             `moadim install`/`uninstall` OS-service registration (linux/macos)
├── sync/                forward sync of managed routines into the OS crontab
├── routines/            routine data model, service layer, command builder, handlers, iCal feed
│
├── utils/
│   ├── time.rs           now_secs() — Unix timestamp helper
│   ├── atomic.rs         atomic_write() — torn-write-safe file writes
│   ├── cron.rs           cron expression normalization/validation
│   ├── fs_perms.rs       create_private_dir_all() — owner-only (0700) directory creation
│   ├── lock.rs           Mutex-poisoning recovery helper
│   ├── process.rs        process-liveness helpers
│   ├── claude_json.rs    prunes a reaped workbench's stale entry from ~/.claude.json
│   └── startup_print.rs  startup banner (REST/MCP/UI URLs)
│
└── build/               build-script modules (compiled by build.rs, not the binary)
    ├── mod.rs
    ├── routine_schema.rs  writes schemas/routine.schema.json + routine.example.toml
    └── client.rs          builds the React client/ app → prebuilt.html / $OUT_DIR/index.html
```

### Filesystem permissions

The daemon's on-disk tree is a secret/transcript store (agent.log transcripts, prompt.md instructions, token-referencing routine state), so on unix it is created **owner-only**:

- Directories under `~/.config/moadim/` are made `0700` via `utils::fs_perms::create_private_dir_all`.
- Files published by `utils::atomic::atomic_write` (routine state, the `prompt.md` sidecar, `machine.local.toml`) are created `0600` before the rename, so they are never briefly world-readable.
- Each routine's launch script sets `umask 077` before its first `mkdir`, so the workbench dir it creates (`0700`) and everything written inside it — the copied `prompt.md`, the appended `CLAUDE.md`, and the tmux-piped `agent.log` — stays unreadable by other local accounts.

Pre-existing files from older installs are tightened on their next write (the modes are not retroactively migrated). Non-unix builds fall back to default permissions.

---

## REST API

Router built in `src/routes/http.rs::build_app`. The full route list is the OpenAPI spec at `apis/openapi.json` (also served live at `/docs/openapi.json`).

Middleware stack (outermost first): `GlobalConcurrencyLimitLayer` → `CatchPanicLayer` → `CompressionLayer` → `logger` → `security_headers` → `host_validation` → `timeout` (the last, `request_timeout`, wraps only the nested `/api/v1` sub-router, so it never applies to the long-lived `/mcp` SSE stream).

---

## MCP transport

`src/routes/mcp.rs` defines `MoadimMcp` with the `#[tool_router]` macro from `rmcp`. Each `#[tool]` method maps to an MCP tool. Tools exposed:

| MCP tool | Delegates to |
|---|---|
| `health` | `FsLocation::current()` + uptime calc |
| `list_routines` | `routines::svc_list` |
| `get_routine` | `routines::svc_get` |
| `create_routine` | `routines::svc_create` |
| `update_routine` | `routines::svc_update` |
| `delete_routine` | `routines::svc_delete` |
| `trigger_routine` | `routines::svc_trigger` |
| `snooze_routine` | `routines::svc_snooze` |
| `set_power_saving` | `routines::svc_set_power_saving` |
| `cleanup_workbenches` | `routines::svc_cleanup` |
| `list_agents` | `routines::available_agents` |
| `create_flag` | `routines::svc_create_flag` |
| `list_flags` | `routines::svc_list_flags` |
| `resolve_flag` | `routines::svc_resolve_flag` |
| `routine_logs` | `routines::svc_logs` |
| `list_routine_runs` | `routines::svc_list_runs` |
| `get_lock_status` | `global_lock::lock_status` |
| `lock_routines` | `global_lock::set_lock` + crontab resync |
| `unlock_routines` | `global_lock::set_lock` + crontab resync |
| `shutdown` | notifies the server's `ShutdownSignal` |
| `restart` | `cli::spawn_restart` |

Transport: `rmcp::transport::streamable_http_server::StreamableHttpService` with `LocalSessionManager`. Each MCP client gets its own session; the `MoadimMcp` handler is cloned per-session with shared `Arc` store and registry.

Connect from Claude Code:
```sh
claude mcp add --transport http moadim http://localhost:5784/mcp
```

---

## Routines — agent-driven jobs (`src/routines/`)

A **routine** is a scheduled job whose payload is an AI agent (claude code, codex, …).
It carries `agent`, `prompt`, `repositories` (`{ repository, branch }`),
and a `title`. Routines have their own store (`RoutineStore`), REST endpoints
(`/routines`), MCP tools (`create_routine`, …), and crontab block.

When a routine fires there is **no moadim process in the loop and no clone step**. At create/update
time moadim writes the raw prompt to `prompts/prompt.pure.md` and composes `prompts/prompt.compiled.local.md`
(a repositories-as-context preamble + the prompt) into `~/.config/moadim/routines/<id>/`, then writes a
single self-contained shell command into a dedicated crontab block:

```
# BEGIN MOADIM-ROUTINES
# Managed by moadim — routines (agent tmux sessions)
<sched> TS=$(date +\%s); WB=…/workbenches/<slug>-$TS; mkdir -p $WB; \
  { cp …/prompts/prompt.compiled.local.md $WB/prompt.md; \
    tmux new-session -d -s moadim-<slug>-$TS -c $WB '<agent-cmd>'; \
    tmux pipe-pane -o -t … "cat >> $WB/agent.log"; } >> $WB/launch.log 2>&1   # moadim-routine:<id>
# END MOADIM-ROUTINES
```

OS cron runs that line directly: it makes a fresh empty workbench under `~/.moadim/workbenches/`,
launches the agent **interactively** (no `-p`) in a detached tmux session rooted there, and captures
output via `pipe-pane`. The prompt reaches the agent as a process **argument** (the `{prompt}`
placeholder expands to `"$(cat prompt.md)"`), so there is no keystroke-injection readiness race. The
agent decides whether to clone the listed repositories. `POST /routines/{id}/trigger` runs the
identical command via `sh -c`.

Everything after the `mkdir` runs inside a `{ … } >> $WB/launch.log 2>&1` group, so a failure in the
prompt copy, the agent's `setup` step, or the `tmux` launch itself is captured next to the run's other
artifacts instead of going to cron's mail spool (silently discarded on the headless hosts this daemon
targets). `agent.log` remains the agent's own output (via `pipe-pane`); `launch.log` is the wrapper's
diagnostics for the steps that get the session running in the first place. Only the `PATH` export and
the `mkdir` itself precede the redirect — a failure that early means `$WB` may not exist yet, so
there's nowhere to write a launch log to.

`agent.log` capture optimizes for **operator readability** over raw audit fidelity: `svc_logs` /
`svc_run_log` (`src/routines/service_log_tail.rs`) strip ANSI/VT escape sequences and collapse
`\r`-based redraw overwrites down to the final on-screen line before serving a tail, and cap the
served window to `MAX_LOG_TAIL_BYTES` (2 MiB, UTF-8-boundary-safe) with a `"N bytes omitted"` marker
rather than the full file. The on-disk file itself is untouched — this is a read-time view, not a
write-time transform — so the raw `pipe-pane` capture remains available on disk for anyone who needs
the byte-exact record; the served view just optimizes for "what is a human looking at right now"
over "what did the terminal literally emit."

Before either path launches, the daemon checks for a live tmux session under the routine's
`moadim-<slug>-` prefix (any `$TS` suffix) and skips the fire — logging a warning instead of
spawning — if one is still running. This overlap guard prevents a run that outlives its schedule
interval from piling up concurrent agent sessions against the same target (duplicate PRs/issues,
racing pushes); see `routines::service_trigger::spawn_routine_command`.

`GET /routines.ics` returns an iCalendar (RFC 5545) feed of every enabled routine's upcoming fire
times (next 30 days, capped per routine), evaluated in the host local timezone. When that zone can
be named, each event's `DTSTART` is `TZID`-qualified with the local wall-clock time against an
embedded `VTIMEZONE` (pinned to the feed's current UTC offset, no DST transition rules), so a
subscriber whose calendar defaults to a different zone still sees the routine's actual configured
local time instead of the same instant reinterpreted in their own zone; when the zone can't be
named, the feed falls back to a bare UTC-instant `DTSTART` with no `VTIMEZONE`. The optional
`?routine=<id>` query param scopes the feed to a single routine (named after it via `X-WR-CALNAME`);
an unknown or disabled id yields a well-formed empty calendar. See `src/routines/ical.rs`.

Finished run workbenches are reaped automatically by a background sweep (every 5 minutes)
(`routines::cleanup`, per-routine `ttl_secs`). `POST /routines/cleanup` (MCP tool
`cleanup_workbenches`) runs that same sweep on demand and returns `{ "removed": N }`, so a caller
need not wait for the next tick. A live tmux session within its run's max runtime is never touched;
the same sweep includes a watchdog that force-kills any session whose run has exceeded the routine's
`max_runtime_secs` (default cap `MAX_RUNTIME_SECS`, 1h) — bounding a hung agent that never exits —
recording the kill in the run's `agent.log`, after which the workbench is reaped under the normal
`ttl_secs` rules.

Because routine agents run in a **detached** tmux session (`tmux new-session -d`, independent of the
daemon process), `moadim stop` / the UI STOP button / `POST /shutdown` used to only stop the daemon's
own HTTP/MCP server, leaving any routine session already running untouched — an in-flight agent could
keep opening PRs, filing issues, pushing commits, etc. until it finished on its own or a later daemon
start's cleanup sweep reaped it via the watchdog above (issue #320). The shutdown path now also drains
those sessions: after the graceful HTTP/MCP shutdown completes, `run_with_listener_until` calls
`routines::cleanup::kill_all_routine_sessions`, which force-kills every still-live
`moadim-{workbench}` tmux session under `~/.moadim/workbenches/` regardless of which routine spawned
it — reusing the same `tmux_session_alive`/`tmux_kill_session` probes and naming convention as the
watchdog and `kill_sessions_for_deleted_routine` (#333), rather than a separate mechanism. A missing
`tmux` binary or no live sessions is a no-op; shutdown is never blocked or failed by it.

TTL reaping bounds age, not total size. `routines::cleanup::disk_cap` adds an optional safety valve
on top of it: if `MOADIM_MAX_WORKBENCH_DISK_BYTES` is set and nonzero, the same sweep sums the whole
`~/.moadim/workbenches/` tree and, once over that ceiling, evicts finished workbenches
oldest-finished-first until back under it — a live session is never touched regardless of size or
age. Unset or `0` preserves the unbounded-by-size behavior above.

The agent command is resolved from a configurable registry at `~/.config/moadim/agents/<name>.toml`
(`command`, `args`; placeholders `{prompt_file}` → `prompt.md`, `{workbench}` → `.`,
`{prompt}` → `"$(cat prompt.md)"`).
The resolved values are baked into the crontab line at sync time, so editing an agent config requires
re-syncing routines that use it. Routines with no matching agent config are skipped (with a warning).

The daemon **owns** the content of a built-in agent config (`claude.toml`, `codex.toml`,
`hermes.toml`), refreshing it from the built-in on every start — the same guarantee
`routines::ensure_default_routines` gives built-in routines — so a shipped fix or improvement reaches
existing installs, not just new ones. A user's edits are still never overwritten: each written config
carries a fingerprint header recording the exact built-in content it was seeded from, and on startup
only a file whose current content still matches that fingerprint (provably untouched since the daemon
wrote it) is refreshed to the current built-in; anything else — an edited file, or one with no
fingerprint at all (seeded before this mechanism existed) — is left alone. See
`routines::agents::ensure_default_agents_in`.

The only placeholders `args` may contain are `{workbench}`, `{prompt_file}`, and `{prompt}`, and at
least one of `{prompt}` / `{prompt_file}` must appear so the agent actually receives the task.
Creating or updating a routine validates the referenced agent's `args` against both rules: an unknown
(typo'd) placeholder token or a missing prompt placeholder is rejected with `400 Bad Request` at edit
time, rather than silently launching the agent with a garbage or empty task at fire time.

Modules: `src/routines/` (model + service + command builder + handlers), `src/routine_storage.rs`
(`routine.toml` + `prompts/prompt.pure.md` + `prompts/prompt.compiled.local.md` persistence),
`src/sync/routines.rs` (the `MOADIM-ROUTINES` block).
Reverse sync (crontab → store) is not implemented for routines.

## Error handling (`src/error.rs`)

```rust
enum AppError {
    Internal,        // 500 — disk I/O failures
    BadRequest(String), // 400 — invalid cron expression
    NotFound,        // 404 — routine ID not in store
    Conflict(String),   // 409 — e.g. a conflicting update
    Locked(String),     // 423 — a global lock sentinel is blocking the operation
}
```

Implements `IntoResponse` → `{"error": "<message>"}` JSON body with matching status code. MCP tools use the `Display` impl of the same error type in a `CallToolResult::error` payload.

---

## Build-time code generation

`build.rs` compiles `src/build/` and runs:

| Step | Output |
|---|---|
| `routine_schema::generate` | `schemas/routine.schema.json` + `schemas/routine.example.toml` |
| `client::build` | `$OUT_DIR/index.html` — React `client/` app, copied as-is (already self-contained via `vite-plugin-singlefile`) |

### UI build strategy

`client::build` runs `pnpm --filter client build` in `client/`. `vite-plugin-singlefile` already inlines the compiled JS and CSS into `client/dist/index.html`, so the build script just:
1. Copies `client/dist/index.html` to `$OUT_DIR/index.html`
2. Copies it to `prebuilt.html` at the package root so `cargo publish` ships it

If `pnpm` is not installed, `prebuilt.html` is used instead. If neither exists, a placeholder page is shown with install instructions.

The prebuilt is stored at the package root — not under `client/` — because `client/` isn't a Cargo workspace member and `cargo publish` would strip it from the tarball otherwise.

---

## Startup sequence

```
main()
  routine_storage::migrate_prompt_files() / migrate_prompts_to_subfolder() / migrate_routine_dirs()
                                          pre-load-time healing
  routine_storage::load_store()          scan ~/.config/moadim/routines/ → RoutineStore
  routines::ensure_default_routines()    seed missing built-in default routines
  routine_storage::repersist_routines()  heal any dirs missing a prompts/ sidecar
  sync::routines::sync_routines_to_crontab()   re-sync the MOADIM-ROUTINES crontab block
  TcpListener::bind(:5784)
  cli::write_pid_file()
  routes::http::run_with_listener_until(routines, listener, termination_signal())
    build_app_with_shutdown(routines, signal)
      AppState { routines, .. }
      StreamableHttpService::new(|| MoadimMcp::new(...))
      Router::new()  ← wire all routes + middleware
    utils::startup_print::print(addr)   stdout: REST / MCP / UI URLs
    axum::serve(listener, app).with_graceful_shutdown(combined)
  cli::clear_pid_file()
```

The server runs until it receives SIGINT/SIGTERM (`termination_signal()`) or the `/shutdown` route
fires its `ShutdownSignal`, whichever comes first.

---

## Concurrency model

- **Single Tokio runtime** (`#[tokio::main]`), all async.
- **`RoutineStore`** is `Arc<Mutex<...>>` — synchronous lock, held only for the duration of the HashMap operation, then released before any disk I/O.
- **MCP sessions** each get a cloned `Arc` of the same store, so mutations from REST and MCP are immediately visible to both.
- **Global routine concurrency cap** (`routines::concurrency_cap`, #335): the per-routine overlap
  guard described above only stops one routine from stacking on its own still-running fire — it
  does nothing to bound how many *different* routines run at once. Since routines fire off the OS
  crontab, many routines' schedules naturally align on the same minute boundary (e.g. `*/5 * * * *`,
  `0 * * * *`), so without a cap a shared tick can launch an unbounded thundering herd of agent
  sessions (CPU/RAM exhaustion, provider API rate-limit bursts). `spawn_routine_command` counts live
  sessions sharing the `moadim-` prefix (`cleanup::tmux_session_count` — derived from actual tmux
  session liveness, not an in-memory counter that could drift after a crash) and, at or over
  `MOADIM_MAX_CONCURRENT_RUNS` (default `0`, meaning unbounded — same convention as
  `MOADIM_MAX_WORKBENCH_DISK_BYTES`), skips the fire with a logged reason instead of launching it or
  queueing it — the simpler, lower-risk policy, matching the overlap guard's own skip-with-warning
  shape rather than adding new queueing infrastructure.

---

## Testing

Tests are colocated: each module `foo.rs` has a companion `foo_tests.rs` and declares it with `#[cfg(test)] #[path = "foo_tests.rs"] mod foo_tests;`. 100% line coverage is enforced by a pre-push git hook.
