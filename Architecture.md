# Moadim Architecture

> _One port to rule them all. Three protocols, one source of truth._
>
> _No moadim in the loop when it counts — the OS does the heavy lifting._

Moadim is a Rust daemon that manages scheduled AI-agent routines and exposes them over two protocols simultaneously — REST and MCP — on a single port (`127.0.0.1:5784`). It also serves an embedded browser UI compiled from a Yew/WASM workspace member.

---

## High-level picture

```
                ┌─────────────────────────────────────────┐
                │           Axum HTTP server :5784         │
                │                                          │
  Browser ──────┤  GET /            (inlined HTML+WASM)   │
  curl/SDK ─────┤  REST /routines   (JSON)                │
  AI agent ─────┤  /mcp             (MCP streamable-HTTP) │
                │                                          │
                │  Shared AppState:                        │
                │    RoutineStore  Arc<Mutex<HashMap>>     │
                └──────────────┬──────────────────────────┘
                               │ read+write on every mutation
                               ▼
               ~/.config/moadim/routines/
               ├── <uuid>/routine.toml      (tracked)
               ├── <uuid>/prompt.md         (tracked)
               ├── <uuid>/run.sh            (generated)
               └── <uuid>/.gitignore        (generated)
```

---

## Source layout

```
src/
├── main.rs              entry point — binds socket, loads store, starts server
├── cli.rs               CLI parsing + background-process lifecycle (status/stop/cleanup/…)
├── commands.rs          data-plane CLI subcommands that drive a running server over HTTP
├── build_info.rs        compile-time build provenance (crate version + git commit/date)
├── error.rs             AppError → HTTP status codes
├── global_lock.rs       lock sentinel that halts all routine scheduling/triggers
├── openapi.rs           utoipa ApiDoc definition served at /docs/openapi.json
├── restart.rs           replaces an already-running daemon with a fresh process
├── routine_storage.rs   routine.toml + prompt.md persistence
│
├── routes/
│   ├── http.rs          Axum router assembly + run_with_listener_until
│   └── mcp.rs           MoadimMcp — rmcp tool_router
│
├── middlewares/
│   ├── logger.rs             request/response logger
│   ├── fs_location.rs        injects x-server-root / x-server-exe-dir headers
│   └── security_headers.rs   adds CSP and related response headers
│
├── filesystem/mod.rs    FsLocation — server working dir + exe dir
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
│   ├── lock.rs           Mutex-poisoning recovery helper
│   ├── process.rs        process-liveness helpers
│   └── startup_print.rs  startup banner (REST/MCP/UI URLs)
│
└── build/               build-script modules (compiled by build.rs, not the binary)
    ├── mod.rs
    ├── openapi.rs       writes apis/openapi.json
    └── ui.rs            runs trunk, inlines WASM → prebuilt.html / $OUT_DIR/index.html

ui/                      Yew workspace member (separate Cargo.toml)
```

---

## REST API

Router built in `src/routes/http.rs::build_app`. The full route list is the OpenAPI spec at `apis/openapi.json` (also served live at `/docs/openapi.json`).

Middleware stack (outermost first): `CompressionLayer` → `logger` → `fs_location` → `security_headers`.

---

## MCP transport

`src/routes/mcp.rs` defines `MoadimMcp` with the `#[tool_router]` macro from `rmcp`. Each `#[tool]` method maps to an MCP tool. Tools exposed:

| MCP tool | Delegates to |
|---|---|
| `health` | `FsLocation::current()` + uptime calc |
| `echo` | inline |
| `list_routines` | `routines::svc_list` |
| `get_routine` | `routines::svc_get` |
| `create_routine` | `routines::svc_create` |
| `update_routine` | `routines::svc_update` |
| `delete_routine` | `routines::svc_delete` |
| `trigger_routine` | `routines::svc_trigger` |
| `cleanup_workbenches` | `routines::svc_cleanup` |
| `list_agents` | `routines::available_agents` |
| `routine_logs` | `routines::svc_logs` |
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
time moadim composes `prompt.md` (a repositories-as-context preamble + the prompt) into
`~/.config/moadim/routines/<id>/`, then writes a single self-contained shell command into a dedicated
crontab block:

```
# BEGIN MOADIM-ROUTINES
# Managed by moadim — routines (agent tmux sessions)
<sched> TS=$(date +\%s); WB=…/workbenches/<slug>-$TS; mkdir -p $WB; cp …/prompt.md $WB/; \
  tmux new-session -d -s moadim-<slug>-$TS -c $WB '<agent-cmd>; echo $? > exit_code'; \
  tmux pipe-pane -o -t … "cat >> $WB/agent.log"   # moadim-routine:<id>
# END MOADIM-ROUTINES
```

OS cron runs that line directly: it makes a fresh empty workbench under `~/.moadim/workbenches/`,
launches the agent **interactively** (no `-p`) in a detached tmux session rooted there, and captures
output via `pipe-pane`. The prompt reaches the agent as a process **argument** (the `{prompt}`
placeholder expands to `"$(cat prompt.md)"`), so there is no keystroke-injection readiness race. The
agent decides whether to clone the listed repositories. `POST /routines/{id}/trigger` runs the
identical command via `sh -c`.

**Per-run exit status.** The agent invocation is suffixed with `; echo $? > exit_code`, so when the
agent process ends, its terminal exit status is recorded into `$WB/exit_code` (the pane's cwd is the
workbench). This makes a finished-but-failed run distinguishable from a successful one: `0` means the
agent exited cleanly, a non-zero value preserves an agent error (crash, auth failure, panic), and the
literal sentinel `killed` (written by the watchdog, below — not `echo`) marks a force-killed run. The
file lives in the workbench and survives until the run is reaped under the normal `ttl_secs` rules.
The capture is the underlying signal for run-outcome consumers (failure alerts, `/metrics` failure
counts, run history); none of those are implemented here.

`GET /routines.ics` returns an iCalendar (RFC 5545) feed of every enabled routine's upcoming fire
times (next 30 days, capped per routine), evaluated in the host local timezone and emitted as UTC
instants so external calendars can subscribe without an embedded `VTIMEZONE`. The optional
`?routine=<id>` query param scopes the feed to a single routine (named after it via `X-WR-CALNAME`);
an unknown or disabled id yields a well-formed empty calendar. See `src/routines/ical.rs`.

Finished run workbenches are reaped automatically by an hourly background sweep
(`routines::cleanup`, per-routine `ttl_secs`). `POST /routines/cleanup` (MCP tool
`cleanup_workbenches`) runs that same sweep on demand and returns `{ "removed": N }`, so a caller
need not wait for the next tick. A live tmux session within its run's max runtime is never touched;
the same sweep includes a watchdog that force-kills any session whose run has exceeded the routine's
`max_runtime_secs` (default cap `MAX_RUNTIME_SECS`, 1h) — bounding a hung agent that never exits —
recording the kill in the run's `agent.log` and writing the `killed` sentinel to its `exit_code`
(see *Per-run exit status* above), after which the workbench is reaped under the normal `ttl_secs`
rules.

The agent command is resolved from a configurable registry at `~/.config/moadim/agents/<name>.toml`
(`command`, `args`; placeholders `{prompt_file}` → `prompt.md`, `{workbench}` → `.`,
`{prompt}` → `"$(cat prompt.md)"`).
The resolved values are baked into the crontab line at sync time, so editing an agent config requires
re-syncing routines that use it. Routines with no matching agent config are skipped (with a warning).

Modules: `src/routines/` (model + service + command builder + handlers), `src/routine_storage.rs`
(`routine.toml` + `prompt.md` persistence), `src/sync/routines.rs` (the `MOADIM-ROUTINES` block).
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
| `openapi::generate` | `apis/openapi.json` — hand-authored OpenAPI 3.0 spec |
| `ui::build` | `$OUT_DIR/index.html` — Yew UI inlined as single file |

### UI inlining strategy

`ui::build` runs `trunk build --release` in the `ui/` workspace member. Trunk emits a `.js` glue file and a `.wasm` binary. The build script then:
1. Base64-encodes the WASM bytes
2. Patches `globalThis.fetch` at runtime so any `*.wasm` request resolves to the inline bytes (avoids touching wasm-bindgen internals)
3. Inlines the JS module and the patched fetch shim into a single `<script type="module">` block
4. Writes the self-contained HTML to `$OUT_DIR/index.html`
5. Copies it to `prebuilt.html` at the package root so `cargo publish` ships it

If `trunk` is not installed, `prebuilt.html` is used instead. If neither exists, a placeholder page is shown with install instructions.

The prebuilt is stored at the package root — not under `ui/` — because `ui/` is a separate workspace member and `cargo publish` would strip it from the tarball.

---

## Startup sequence

```
main()
  routine_storage::migrate_prompt_files() / migrate_routine_dirs()   pre-load-time healing
  routine_storage::load_store()          scan ~/.config/moadim/routines/ → RoutineStore
  routines::ensure_default_routines()    seed missing built-in default routines
  routine_storage::repersist_routines()  heal any dirs missing a prompt.md sidecar
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

---

## Testing

Tests are colocated: each module `foo.rs` has a companion `foo_tests.rs` and declares it with `#[cfg(test)] #[path = "foo_tests.rs"] mod foo_tests;`. 100% line coverage is enforced by a pre-push git hook.
