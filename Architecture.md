# Moadim Architecture

> _One port to rule them all. Three protocols, one source of truth._
>
> _No moadim in the loop when it counts — the OS does the heavy lifting._

Moadim is a Rust daemon that manages cron jobs and exposes them over two protocols simultaneously — REST and MCP — on a single port (`127.0.0.1:5784`). It also serves an embedded browser UI compiled from a Yew/WASM workspace member.

---

## High-level picture

A single Axum HTTP server listens on port `5784` and fans incoming requests out to three surfaces from one process: browsers hit `GET /ui` (inlined HTML + WASM), `curl`/SDK clients use the REST `/cron-jobs` endpoints (JSON), and AI agents connect to `/mcp` (MCP streamable-HTTP). All three share a single `AppState` that holds the `CronStore` (an `Arc<Mutex<HashMap>>`) and the `HandlerRegistry` (an `Arc<HashSet>`). Every mutation reads and writes through that shared state, which in turn persists to `~/.config/moadim/jobs/`, where each job lives under its own UUID directory containing a tracked `job.toml`, a gitignored `job.local.toml` for local overrides, and a `.gitignore`.

---

## Source layout

The crate is rooted at `src/`. `main.rs` is the entry point that binds the socket, loads the store, and starts the server; `lib.rs` is the library root that re-exports the wasm module when targeting `wasm32`.

The top-level modules are: `cron_jobs.rs` (data model, service layer, and Axum HTTP handlers); `storage.rs` (TOML persistence — load, write, remove); `system_cron.rs` (read-only discovery of host cron jobs); `fs_location.rs` (captures the working dir and exe dir for response headers); `paths/mod.rs` (path builders for `~/.config/moadim/jobs/`); `error.rs` (maps `AppError` to HTTP status codes); `banner.rs` (startup banner); and `wasm.rs` (wasm-bindgen exports for the browser side).

Submodules group the remaining concerns:

- `routes/` — `http.rs` (Axum router assembly plus `run_with_listener_until`) and `mcp.rs` (`MoadimMcp`, the rmcp `tool_router`).
- `middlewares/` — `logger.rs` (request/response logger) and `fs_location.rs` (injects the `x-server-root` and `x-server-exe-dir` headers).
- `utils/` — `time.rs` (`now_secs()`, a Unix timestamp helper) and `schema.rs` (a schemars override for the free-form JSON metadata field).
- `build/` — build-script modules compiled by `build.rs` rather than into the binary: `mod.rs`, `openapi.rs` (writes `apis/openapi.json`), `job_schema.rs` (writes `schemas/job.schema.json` and `job.example.toml`), and `ui.rs` (runs trunk and inlines WASM into `prebuilt.html` / `$OUT_DIR/index.html`).

Outside `src/`, `ui/` is a Yew workspace member with its own `Cargo.toml`, and `tests/` holds integration tests.

---

## Core types

### `CronJob` (`src/cron_jobs.rs`)

`CronJob` is the core managed-job record. Its fields are: `id` (a UUID v4 string); `schedule` (the cron expression); `handler` (a name under `~/.config/moadim/handlers/`); `metadata` (an arbitrary JSON object, `serde_json::Value`); `enabled` (bool); `source` (one of `"managed"`, `"system:user-crontab"`, `"system:etc-crontab"`, or `"system:cron.d/<file>"`); `created_at` and `updated_at` (Unix seconds); and `last_manual_trigger_at` (an optional Unix timestamp covering manual triggers only — scheduled fires do not update it). The `last_manual_trigger_at` field carries a serde alias for the old `last_triggered_at` key so pre-rename records still deserialize.

> `last_manual_trigger_at` is project-wide: both `CronJob` and `Routine` (`src/routines/model.rs`) carry it, renamed from `last_triggered_at` and kept readable via the `#[serde(alias = "last_triggered_at")]` back-compat alias.

### `CronJobResponse`

`CronJob` + `handler_registered: bool` + `file_path: String`. Returned by all managed-job endpoints. `handler_registered` is true when the job's `handler` string appears in `HandlerRegistry`.

### `CronStore` / `HandlerRegistry`

`CronStore` is a type alias for `Arc<Mutex<HashMap<String, CronJob>>>` and `HandlerRegistry` is a type alias for `Arc<HashSet<String>>`.

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
| `svc_trigger` | Records `last_manual_trigger_at = now` (**manual** triggers only — scheduled cron firings run the built command directly and never update it), rewrites TOML |
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

Connect from Claude Code by adding the server as an HTTP transport named `moadim` pointing at `http://localhost:5784/mcp` (via `claude mcp add --transport http`).

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

Each job lives in its own UUID directory under `~/.config/moadim/jobs/`. That directory holds `job.toml` (schedule, handler, enabled flag, timestamps, and a `[metadata]` table); `job.local.toml` (the same schema, overriding `job.toml`, and gitignored); an auto-created `.gitignore` covering `*.local.*` and `*.log`; and `job.local.log` (the runtime log, also gitignored).

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
crontab block. That block is delimited by `# BEGIN MOADIM-ROUTINES` and `# END MOADIM-ROUTINES` markers and contains a single scheduled line. The line stamps the current epoch seconds into a per-run workbench path, creates that workbench directory, copies `prompt.txt` into it, launches the agent in a detached tmux session rooted at the workbench, and wires `tmux pipe-pane` so the session's output is appended to `agent.log`. The line is tagged with a trailing `moadim-routine:<id>` comment so it can be located and replaced on re-sync.

OS cron runs that line directly: it makes a fresh empty workbench under `~/.moadim/workbenches/`,
launches the agent **interactively** (no `-p`) in a detached tmux session rooted there, and captures
output via `pipe-pane`. The prompt reaches the agent as a process **argument** (the `{prompt}`
placeholder expands to `"$(cat prompt.txt)"`), so there is no keystroke-injection readiness race. The
agent decides whether to clone the listed repositories. `POST /routines/{id}/trigger` runs the
identical command via `sh -c`.

`GET /routines.ics` returns an iCalendar (RFC 5545) feed of every enabled routine's upcoming fire
times (next 30 days, capped per routine), evaluated in the host local timezone and emitted as UTC
instants so external calendars can subscribe without an embedded `VTIMEZONE`. See `src/routines/ical.rs`.

Finished run workbenches are reaped automatically by an hourly background sweep
(`routines::cleanup`, per-routine `ttl_secs`). `POST /routines/cleanup` (MCP tool
`cleanup_workbenches`) runs that same sweep on demand and returns `{ "removed": N }`, so a caller
need not wait for the next tick. A live tmux session within its run's max runtime is never touched;
the same sweep includes a watchdog that force-kills any session whose run has exceeded the routine's
`max_runtime_secs` (default cap `MAX_RUNTIME_SECS`, 1h) — bounding a hung agent that never exits —
recording the kill in the run's `agent.log`, after which the workbench is reaped under the normal
`ttl_secs` rules.

The agent command is resolved from a configurable registry at `~/.config/moadim/agents/<name>.toml`
(`command`, `args`; placeholders `{prompt_file}` → `prompt.txt`, `{workbench}` → `.`,
`{prompt}` → `"$(cat prompt.txt)"`).
The resolved values are baked into the crontab line at sync time, so editing an agent config requires
re-syncing routines that use it. Routines with no matching agent config are skipped (with a warning).

Modules: `src/routines.rs` (model + service + command builder + handlers), `src/routine_storage.rs`
(`routine.toml` + `prompt.txt` persistence), `src/sync/routines.rs` (the `MOADIM-ROUTINES` block).
Reverse sync (crontab → store) is not implemented for routines.

## Error handling (`src/error.rs`)

The `AppError` enum has three variants: `Internal` (HTTP 500, for disk I/O failures), `BadRequest(String)` (HTTP 400, for an invalid cron expression), and `NotFound` (HTTP 404, for a job ID not present in the store).

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

On startup, `main()` first calls `storage::load_store()` to scan `~/.config/moadim/jobs/` and build the `CronStore`, then binds a `TcpListener` on port `5784`. It hands the store and listener to `routes::http::run_with_listener_until` along with a never-resolving shutdown future. That function calls `build_app(store)`, which constructs the `AppState` (the store plus a fresh handler registry), wires up the MCP `StreamableHttpService` (each session built via `MoadimMcp::new(...)`), and assembles the `Router` with all routes and middleware. The banner is then printed to stdout (the REST, MCP, and UI URLs), and `axum::serve` runs the app with graceful shutdown bound to that pending future.

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
