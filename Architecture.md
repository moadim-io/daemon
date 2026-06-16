# Moadim Architecture

> _One port to rule them all. Three protocols, one source of truth._
>
> _No moadim in the loop when it counts — the OS does the heavy lifting._

Moadim is a Rust daemon that manages cron jobs and exposes them over two protocols simultaneously — REST and MCP — on a single port (`127.0.0.1:5784`). It also serves an embedded browser UI compiled from a Yew/WASM workspace member.

---

## High-level picture

```
                ┌─────────────────────────────────────────┐
                │           Axum HTTP server :5784         │
                │                                          │
  Browser ──────┤  GET /ui          (inlined HTML+WASM)   │
  curl/SDK ─────┤  REST /cron-jobs  (JSON)                │
  AI agent ─────┤  /mcp             (MCP streamable-HTTP) │
                │                                          │
                │  Shared AppState:                        │
                │    CronStore   Arc<Mutex<HashMap>>       │
                │    HandlerRegistry  Arc<HashSet>         │
                └──────────────┬──────────────────────────┘
                               │ read+write on every mutation
                               ▼
               ~/.config/moadim/jobs/
               ├── <uuid>/job.toml          (tracked)
               ├── <uuid>/job.local.toml    (gitignored, local overrides)
               └── <uuid>/.gitignore
```

---

## Source layout

```
src/
├── main.rs              entry point — binds socket, loads store, starts server
├── lib.rs               library root — re-exports wasm module when target=wasm32
│
├── cron_jobs.rs         data model + service layer + Axum HTTP handlers
├── storage.rs           TOML persistence (load / write / remove)
├── system_cron.rs       read-only discovery of host cron jobs
├── fs_location.rs       captures working dir + exe dir for response headers
├── paths/mod.rs         path builders for ~/.config/moadim/jobs/
├── error.rs             AppError → HTTP status codes
├── banner.rs            startup banner
├── wasm.rs              wasm-bindgen exports (browser-side)
│
├── routes/
│   ├── http.rs          Axum router assembly + run_with_listener_until
│   └── mcp.rs           MoadimMcp — rmcp tool_router
│
├── middlewares/
│   ├── logger.rs        request/response logger
│   └── fs_location.rs   injects x-server-root / x-server-exe-dir headers
│
├── utils/
│   ├── time.rs          now_secs() — Unix timestamp helper
│   └── schema.rs        schemars override for free-form JSON metadata field
│
└── build/               build-script modules (compiled by build.rs, not the binary)
    ├── mod.rs
    ├── openapi.rs       writes apis/openapi.json
    ├── job_schema.rs    writes schemas/job.schema.json + job.example.toml
    └── ui.rs            runs trunk, inlines WASM → prebuilt.html / $OUT_DIR/index.html

ui/                      Yew workspace member (separate Cargo.toml)
tests/                   integration tests
```

---

## Core types

### `CronJob` (`src/cron_jobs.rs`)

```rust
pub struct CronJob {
    pub id: String,                       // UUID v4
    pub schedule: String,                 // cron expression
    pub handler: String,                  // name in ~/.config/moadim/handlers/
    pub metadata: serde_json::Value,      // arbitrary JSON object
    pub enabled: bool,
    pub source: String,                   // "managed" | "system:user-crontab" | "system:etc-crontab" | "system:cron.d/<file>"
    pub created_at: u64,                  // Unix seconds
    pub updated_at: u64,
    pub last_triggered_at: Option<u64>,
}
```

### `CronJobResponse`

`CronJob` + `handler_registered: bool` + `file_path: String`. Returned by all managed-job endpoints. `handler_registered` is true when the job's `handler` string appears in `HandlerRegistry`.

### `CronStore` / `HandlerRegistry`

```rust
pub type CronStore       = Arc<Mutex<HashMap<String, CronJob>>>;
pub type HandlerRegistry = Arc<HashSet<String>>;
```

Both are cloned into `AppState` (REST) and `MoadimMcp` (MCP). Every write acquires the mutex, updates in memory, then flushes to disk before releasing.

---

## Service layer

`src/cron_jobs.rs` exposes six functions that contain all business logic:

| Function | What it does |
|---|---|
| `svc_list` | Returns all jobs sorted by `created_at` |
| `svc_get` | Looks up one job, `NotFound` if absent |
| `svc_create` | Validates cron expr, assigns UUID v4, writes TOML, inserts into store |
| `svc_update` | Partial-updates fields, bumps `updated_at`, rewrites TOML |
| `svc_delete` | Removes from store, deletes job directory |
| `svc_trigger` | Records `last_triggered_at = now`, rewrites TOML |
| `svc_logs_path` | Checks job exists, returns path to `job.local.log` |

Both the HTTP handlers and MCP tools call these directly — there is no duplication of logic between the two transports.

---

## REST API

Router built in `src/routes/http.rs::build_app`. The full route list is the OpenAPI spec at `apis/openapi.json` (also served live at `/docs/openapi.json`).

Middleware stack (outermost first): `logger` → `fs_location`.

---

## MCP transport

`src/routes/mcp.rs` defines `MoadimMcp` with the `#[tool_router]` macro from `rmcp`. Each `#[tool]` method maps to an MCP tool. Tools exposed:

| MCP tool | Delegates to |
|---|---|
| `health` | `FsLocation::current()` + uptime calc |
| `echo` | inline |
| `list_cron_jobs` | `svc_list` |
| `list_system_cron_jobs` | `system_cron::read_all` |
| `get_cron_job` | `svc_get` |
| `create_cron_job` | `svc_create` |
| `update_cron_job` | `svc_update` |
| `delete_cron_job` | `svc_delete` |
| `trigger_cron_job` | `svc_trigger` |

Transport: `rmcp::transport::streamable_http_server::StreamableHttpService` with `LocalSessionManager`. Each MCP client gets its own session; the `MoadimMcp` handler is cloned per-session with shared `Arc` store and registry.

Connect from Claude Code:
```sh
claude mcp add --transport http moadim http://localhost:5784/mcp
```

---

## Persistence (`src/storage.rs`)

### On startup

`storage::load_store()` scans `~/.config/moadim/jobs/`. For each subdirectory it:
1. Reads `job.toml` (required)
2. Reads `job.local.toml` (optional override — local values win field-by-field)
3. Merges metadata tables (local keys overwrite base keys)
4. Constructs a `CronJob` with `source = "managed"`

Invalid or missing `job.toml` → directory silently skipped.

### On write

`storage::write_job` creates the job directory if absent, writes a fresh `.gitignore` (`*.local.*\n*.log\n`) if none exists, then serializes to `job.toml`. The `.gitignore` ensures secrets in `job.local.toml` and logs are never accidentally committed.

### File layout

```
~/.config/moadim/jobs/
└── <uuid>/
    ├── job.toml         schedule, handler, enabled, timestamps, [metadata]
    ├── job.local.toml   same schema, overrides job.toml (gitignored)
    ├── .gitignore       auto-created: *.local.* and *.log
    └── job.local.log    runtime log (gitignored)
```

Cron expression uses standard 5-field syntax (`min hour dom month dow`). The `cron` crate requires 7 fields internally; `normalize_cron` pads 5-field input to 7 before validation.

---

## System cron discovery (`src/system_cron.rs`)

Read-only. Called on-demand by `GET /system-cron-jobs` and the `list_system_cron_jobs` MCP tool. Not stored in `CronStore`.

Sources checked in order:
1. `crontab -l` → `source = "system:user-crontab"` (no user field)
2. `/etc/crontab` → `source = "system:etc-crontab"` (has user field)
3. `/etc/cron.d/<file>` → `source = "system:cron.d/<file>"` (has user field)

Handles both standard 5-field syntax and `@keyword` shortcuts. IDs are deterministic hashes of `(source, schedule, command)` so they are stable across calls. Lines starting with `#`, blank lines, and `KEY=value` env-var lines are skipped.

---

## Routines — agent-driven jobs (`src/routines.rs`)

A **routine** is a second kind of scheduled job whose payload is an AI agent (claude code, codex, …)
instead of a handler script. It carries `agent`, `prompt`, `repositories` (`{ repository, branch }`),
and a `title`. Routines are a separate type with their own store (`RoutineStore`), REST endpoints
(`/routines`), MCP tools (`create_routine`, …), and crontab block — they do not share `CronJob`.

When a routine fires there is **no moadim process in the loop and no clone step**. At create/update
time moadim composes `prompt.txt` (a repositories-as-context preamble + the prompt) into
`~/.config/moadim/routines/<id>/`, then writes a single self-contained shell command into a dedicated
crontab block:

```
# BEGIN MOADIM-ROUTINES
# Managed by moadim — routines (agent tmux sessions)
<sched> TS=$(date +\%s); WB=…/workbenches/<slug>-$TS; mkdir -p $WB; cp …/prompt.txt $WB/; \
  tmux new-session -d -s moadim-<slug>-$TS -c $WB '<agent-cmd>'; \
  tmux pipe-pane -o -t … "cat >> $WB/agent.log"   # moadim-routine:<id>
# END MOADIM-ROUTINES
```

OS cron runs that line directly: it makes a fresh empty workbench under `~/.moadim/workbenches/`,
launches the agent **interactively** (no `-p`) in a detached tmux session rooted there, and captures
output via `pipe-pane`. The prompt reaches the agent as a process **argument** (the `{prompt}`
placeholder expands to `"$(cat prompt.txt)"`), so there is no keystroke-injection readiness race. The
agent decides whether to clone the listed repositories. `POST /routines/{id}/trigger` runs the
identical command via `sh -c`.

Finished run workbenches are reaped automatically by an hourly background sweep
(`routines::cleanup`, per-routine `ttl_secs`). `POST /routines/cleanup` (MCP tool
`cleanup_workbenches`) runs that same sweep on demand and returns `{ "removed": N }`, so a caller
need not wait for the next tick. Live tmux sessions are never touched.

The agent command is resolved from a configurable registry at `~/.config/moadim/agents/<name>.toml`
(`command`, `args`; placeholders `{prompt_file}` → `prompt.txt`, `{workbench}` → `.`,
`{prompt}` → `"$(cat prompt.txt)"`).
The resolved values are baked into the crontab line at sync time, so editing an agent config requires
re-syncing routines that use it. Routines with no matching agent config are skipped (with a warning).

Modules: `src/routines.rs` (model + service + command builder + handlers), `src/routine_storage.rs`
(`routine.toml` + `prompt.txt` persistence), `src/sync/routines.rs` (the `MOADIM-ROUTINES` block).
Reverse sync (crontab → store) is not implemented for routines.

## Error handling (`src/error.rs`)

```rust
enum AppError {
    Internal,           // 500 — disk I/O failures
    BadRequest(String), // 400 — invalid cron expression
    NotFound,           // 404 — job ID not in store
}
```

Implements `IntoResponse` → `{"error": "<message>"}` JSON body with matching status code. MCP tools use the `Display` impl of the same error type in a `CallToolResult::error` payload.

---

## Build-time code generation

`build.rs` compiles `src/build/` and runs three steps:

| Step | Output |
|---|---|
| `openapi::generate` | `apis/openapi.json` — hand-authored OpenAPI 3.0 spec |
| `job_schema::generate` | `schemas/job.schema.json`, `schemas/job.example.toml` |
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

## WASM module (`src/wasm.rs`)

When compiled for `target_arch = "wasm32"`, the binary becomes a WASM module with `wasm-bindgen` exports:

| Export | Description |
|---|---|
| `wasm_init()` | Initialize `console_log` |
| `wasm_query_health()` | `GET /health` → JSON string |
| `wasm_echo(message)` | `POST /echo` → JSON string |
| `wasm_get_info()` | `GET /info` → JSON string |
| `wasm_mode()` | Returns `"wasm"` |
| `wasm_checksum(input)` | DJB2 hash → hex string |
| `wasm_reverse(input)` | Reversed string |
| `wasm_uppercase(input)` | Uppercased string |

These are the bindings called by the Yew UI to communicate with the native server.

---

## Startup sequence

```
main()
  storage::load_store()          scan ~/.config/moadim/jobs/ → CronStore
  TcpListener::bind(:5784)
  routes::http::run_with_listener_until(store, listener, pending())
    build_app(store)
      AppState { store, handlers: new_registry() }
      StreamableHttpService::new(|| MoadimMcp::new(...))
      Router::new()  ← wire all routes + middleware
    banner::print(addr)          stdout: REST / MCP / UI URLs
    axum::serve(listener, app).with_graceful_shutdown(pending())
```

`std::future::pending()` means the server runs until the process is killed.

---

## Concurrency model

- **Single Tokio runtime** (`#[tokio::main]`), all async.
- **`CronStore`** is `Arc<Mutex<...>>` — synchronous lock, held only for the duration of the HashMap operation, then released before any disk I/O.
- **`HandlerRegistry`** is `Arc<HashSet<...>>` — read-only after creation, no locking needed.
- **MCP sessions** each get a cloned `Arc` of the same store, so mutations from REST and MCP are immediately visible to both.

---

## Testing

Tests are colocated: each module `foo.rs` has a companion `foo_tests.rs` and declares it with `#[cfg(test)] #[path = "foo_tests.rs"] mod foo_tests;`. 100% line coverage is enforced by a pre-push git hook.
