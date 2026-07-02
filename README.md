# moadim

[![crates.io](https://img.shields.io/crates/v/moadim.svg)](https://crates.io/crates/moadim)
[![docs.rs](https://img.shields.io/docsrs/moadim)](https://docs.rs/moadim)
[![CI](https://img.shields.io/github/actions/workflow/status/moadim-io/daemon/test.yml?branch=main&label=CI)](https://github.com/moadim-io/daemon/actions/workflows/test.yml)
[![License: MIT](https://img.shields.io/crates/l/moadim.svg)](https://opensource.org/licenses/MIT)

> **Loop engineering, on a schedule.** Stop prompting your agents — design the loop that prompts them.
>
> **Agent routines that run while you sleep.** One port. Three interfaces. Zero drift.
>
> _Set the loop. Forget the keyboard. moadim fires the prompt so you don't have to._

**One-line install** — install Rust/Cargo, install moadim, then run it:

```sh
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y && . "$HOME/.cargo/env" && cargo install --locked moadim && moadim
```

Rust server that schedules **routines** (run an AI agent on a cron schedule),
exposing them over three interfaces simultaneously:

- **UI** (`http://localhost:5784/`) — browser dashboard for managing routines
- **REST** (`http://localhost:5784/api/v1`) — standard HTTP API for browsers, CLI tools, and services; Swagger UI at `/docs`
- **MCP** (`http://localhost:5784/mcp`) — [Model Context Protocol](https://modelcontextprotocol.io) for AI agents (Claude, etc.)

All three share the same port. Routines created through any interface are
automatically synced to the OS crontab so they actually run on schedule. See
[Routines](#routines) for the agent-loop engine, or
[`docs/comparison.md`](docs/comparison.md) for how moadim compares to cron,
GitHub Actions, and other agent runners.

## Prerequisites

moadim depends on a few external tools at runtime, in addition to Rust/Cargo for
building and installing:

| Tool       | Required for | Install |
| ---------- | ------------ | ------- |
| Rust/Cargo | building and installing moadim | <https://rustup.rs> |
| `tmux`     | launching routine agents — every scheduled routine starts its agent inside a tmux session. **Without `tmux`, routine runs silently fail to launch.** | `brew install tmux` (macOS) · `apt install tmux` (Debian/Ubuntu) |
| `crontab`  | scheduling — moadim writes managed routines into the OS crontab so they fire on schedule | preinstalled on macOS; `apt install cron` (Debian/Ubuntu) |

The daemon reports whether `tmux` resolves on its `PATH` in `GET /api/v1/health`
(under `dependencies`) and logs a warning at startup when it is missing, so a
misconfigured host is easy to spot.

## Installation

```sh
cargo install --locked moadim
```

`--locked` installs the exact dependency graph published and tested with this
release (from the crate's `Cargo.lock`) instead of re-resolving every dependency
to the newest semver-compatible version at install time — so a bad or breaking
transitive bump can't fail an otherwise-unchanged install.

If `moadim` is not found after install, add Cargo's bin directory to your PATH:

```sh
echo 'export PATH="$HOME/.cargo/bin:$PATH"' >> ~/.zshrc && source ~/.zshrc
```

Then run:

```sh
moadim
```

This starts the server **in the background** and returns control to your shell.
Stop it later with `moadim stop` (or the STOP button in the UI). To run it
attached to your terminal instead, use `moadim --interactive`.

### Man page

A Unix man page ships in [`docs/moadim.1`](docs/moadim.1), mirroring the
built-in `moadim --help`. View it without installing:

```sh
man ./docs/moadim.1
```

Or install it so `man moadim` works system-wide (packagers can drop it into
`share/man/man1/`):

```sh
install -Dm644 docs/moadim.1 "$HOME/.local/share/man/man1/moadim.1"
```

## Features

> _Close the loop. Skip the keyboard. Loop engineering, shipped as a daemon._

- Routines created via REST or MCP are written into your OS crontab automatically
- **Routines** schedule an AI agent — a prompt + schedule + agent, stored in `~/.config/moadim/routines/` (see [Routines](#routines)) — git-trackable, diff-friendly
- **Agents** are a registry of coding agents (`claude`, …) under `~/.config/moadim/agents/<name>.toml`, referenced by routines
- Each routine run executes in a throwaway **workbench** under `~/.moadim/workbenches/` (a separate tree from the config dir), reaped on an hourly cleanup sweep
- Same REST and MCP interface — no logic duplication between protocols
- API spec auto-generated at build time into `apis/`

## Directory layout

```
~/.config/moadim/
├── routines/                  # scheduled AI-agent tasks (see ## Routines)
│   └── nightly-triage/
│       ├── routine.toml       # tracked — schedule, agent, prompt, repositories
│       ├── prompt.md          # tracked — the rendered prompt handed to the agent
│       ├── run.sh             # generated — the crontab entry invokes this
│       └── .gitignore         # generated — excludes *.local.* and *.log
├── agents/                    # registered coding agents referenced by routines
│   └── claude.toml
└── user_prompt.md             # optional — appended to every routine's prompt (see ## Routines)

~/.moadim/                     # runtime tree, separate from the config dir above
└── workbenches/               # per-run throwaway dirs, reaped on the hourly sweep
```

## Crontab sync

> _Your crontab, your rules — moadim keeps its own block in sync._

Moadim owns a single block inside your crontab for routines. Everything outside that block is untouched.

```
# BEGIN MOADIM-ROUTINES
# Managed by moadim — routines (agent tmux sessions)
* * * * * /…/moadim schedule trigger '<id>' # moadim-routine:<id>
# END MOADIM-ROUTINES
```

**Forward sync (moadim → crontab):** any time you create, update, or delete a routine via the UI, REST, or MCP, the crontab block is rewritten immediately. Disabled routines are excluded from the block.

**Reverse sync (crontab → moadim) is not implemented.** Edit routines through the UI, REST, or MCP rather than by hand: manual changes inside the block do **not** sync back into moadim and are overwritten by the next forward sync.

**Schedule format:** standard 5-field cron (`min hour dom month dow`), same as the OS crontab. `@keyword` shortcuts (`@hourly`, `@daily`, `@weekly`, `@monthly`, `@yearly`, `@annually`) are also accepted. `@reboot` and `@midnight` are **not** supported via the API and are rejected with `400 Bad Request`.

**Timezone:** because routines run via the OS crontab, schedules are evaluated in the host's **local system timezone**, not UTC. A schedule of `0 9 * * *` fires at 09:00 local time. AI agents in particular should not pre-convert times to UTC.

## Routines

A **routine** is a scheduled AI-agent task: it fires a prompt at a coding
agent (e.g. Claude) on a cron schedule, each run inside its own throwaway
workbench.

Routines are stored as folders under `~/.config/moadim/routines/<id>/`,
git-trackable:

```
~/.config/moadim/routines/
└── nightly-triage/
    ├── routine.toml   # tracked — schedule, agent, prompt, repositories
    ├── prompt.md      # tracked — the rendered prompt handed to the agent
    ├── run.sh         # generated — the crontab entry invokes this
    └── .gitignore     # generated — excludes *.local.* and *.log
```

| Field          | Type   | Required | Description                                                                                  |
| -------------- | ------ | -------- | -------------------------------------------------------------------------------------------- |
| `schedule`     | string | yes      | Cron expression (`min hour dom month dow` or `@daily`, …), evaluated in the host's local timezone — **not** UTC. |
| `title`        | string | yes      | Human name; slugified to name the run workbench and tmux session.                            |
| `agent`        | string | yes      | Agent registry key (e.g. `claude`), resolved from `~/.config/moadim/agents/<agent>.toml`.    |
| `prompt`       | string | yes      | The task prompt handed to the agent.                                                          |
| `repositories` | list   | no       | Git repos listed in the prompt as context. Moadim does **not** clone them — the agent does.   |
| `enabled`      | bool   | no       | Defaults to `true`. Set `false` to pause without deleting.                                    |
| `ttl_secs`     | int    | no       | How long a finished run's workbench is retained before auto-cleanup. Caps the cron-derived retention lower — it can only shorten, never extend it. `None` uses the cron-derived value. |
| `max_runtime_secs` | int | no       | Max wall-clock seconds a single run may execute before the cleanup watchdog force-kills its (hung) tmux session; the workbench is then reaped under the normal TTL rules. Caps the cron-derived runtime (`min(MAX_RUNTIME_SECS, cron interval)`) lower — it can only shorten, never extend it. `None` uses the cron-derived value. |

**Workbenches and cleanup:** each run executes in a workbench under
`~/.moadim/workbenches/`. Finished, expired workbenches are reaped on an
hourly sweep so they don't accumulate; trigger a sweep on demand with
`moadim cleanup`. Sessions still running are never reaped.

**REST** — under the `/api/v1` prefix:

```
GET    /routines              # list (filter by ?repository=, sort by ?sort=/&order=)
POST   /routines              # create
GET    /routines/{id}         # fetch one
PUT    /routines/{id}         # replace
PATCH  /routines/{id}         # update fields
DELETE /routines/{id}         # delete
POST   /routines/{id}/trigger # run now, outside the schedule
GET    /routines/{id}/logs    # run output
POST   /routines/cleanup      # reap expired workbenches now
GET    /agents                # list registered agents
GET    /routines.ics          # subscribe to fire times as a calendar feed
```

**MCP** — the same operations are exposed as tools: `list_routines`,
`get_routine`, `create_routine`, `update_routine`, `delete_routine`,
`trigger_routine`, `routine_logs`, `list_agents`, and `cleanup_workbenches`.

**Agents:** the `agent` field resolves to a config at
`~/.config/moadim/agents/<agent>.toml`. API responses include
`agent_registered` so callers can tell whether the named agent is configured on
the host.

**Built-in `claude` agent prerequisites:** the default `claude` agent needs two
things on the host beyond the `claude` CLI itself:

- **`python3`** — the agent's `setup` step runs a short `python3` snippet to
  pre-seed per-workbench state in `~/.claude.json` (trust dialog + MCP-server
  approvals) so the unattended session never blocks on a prompt. If `python3` is
  not on `PATH`, the setup step fails and the run no-ops — the routine still
  shows a healthy (green) status, but the agent never actually launches.
- **`tmux`** — every routine run is launched inside a tmux session (named after
  the run's workbench), so a tmux binary must be installed.

Both are present by default on most developer machines; install them explicitly
on a minimal host (e.g. a CI runner or fresh container) before relying on the
built-in `claude` agent.

### Agent configuration

Each agent is a single TOML file at `~/.config/moadim/agents/<name>.toml`, where
`<name>` is the registry key a routine's `agent` field references (the filename
stem, e.g. `claude.toml` → `claude`). On startup the daemon seeds the built-in
defaults (`claude`, `codex`, `hermes`) into this directory **only if the file is
absent** — your edits are never overwritten — so you can both tweak a default and
register a brand-new agent by dropping in another `<name>.toml`.

| Field     | Type           | Required | Description                                                                                                                   |
| --------- | -------------- | -------- | ----------------------------------------------------------------------------------------------------------------------------- |
| `command` | string         | yes      | Executable to run (resolved on `PATH`), e.g. `"claude"`.                                                                       |
| `args`    | array<string>  | no       | Arguments passed to `command`. Supports the placeholders below. Defaults to empty.                                            |
| `setup`   | string         | no       | Shell command run in the workbench **before** the agent launches, inserted verbatim into the cron line. See the variables below. |

**Placeholders** (substituted in each `args` entry at launch):

- `{workbench}` — absolute path to the run's workbench directory.
- `{prompt_file}` — path to the composed prompt file (the rendered `CLAUDE.md`
  plus the routine's `prompt`). Pass this when the CLI reads its prompt from a
  file (e.g. `codex exec {prompt_file}`).
- `{prompt}` — the composed prompt inlined as a single shell-quoted argument.
  Pass this when the CLI takes the prompt as a positional argument.

**`setup` variables** — the `setup` command runs with two shell variables in
scope, so it can prepare per-run state before the agent starts:

- `$WB` — absolute workbench path.
- `$SESS` — the tmux session name for the run.

Examples — the headless `codex`/`hermes` form, and the interactive `claude` form
(the real default's `setup` step also pre-seeds `~/.claude.json`; see the
prerequisites above):

```toml
# ~/.config/moadim/agents/codex.toml
command = "codex"
args = ["exec", "{prompt_file}"]
```

```toml
# ~/.config/moadim/agents/claude.toml
command = "claude"
args = ["--permission-mode", "auto", "{prompt}"]
# setup = '''...optional pre-launch shell command, runs with $WB and $SESS in scope...'''
```

A routine whose `agent` names a file that is missing or whose TOML is malformed
fails to launch; `GET /routines` reports `agent_registered: false` for the former
so you can spot an unconfigured agent before it fires.

### Global user prompt

An optional `~/.config/moadim/user_prompt.md` lets you inject persistent,
host-wide instructions into **every** routine. At launch each run renders a
`CLAUDE.md` in its workbench from two layers:

1. **Moadim preamble** — the daemon-managed header, the routine-origin
   disclosure (naming the routine), and the run-date/timezone stamp.
2. **Your user prompt** — if `user_prompt.md` exists, its contents are appended
   below a `---` separator.

Use it for standing guidance that should apply to all routines regardless of
their individual `prompt` — coding conventions, who to tag, tone, or a default
identity. It is purely additive: a missing file is skipped silently, and it
never replaces a routine's own `prompt`. Because it is user-scope (not per
routine), it lives at the config root rather than inside a routine folder.

## Running

Moadim runs as a local daemon. By default it starts **in the background**:

```sh
moadim                 # start detached, print the PID, return to the shell
moadim --interactive   # run in the foreground, attached to the terminal (Ctrl-C to stop)
moadim status          # report whether a server is running
moadim status --json   # same, as a machine-readable JSON object
moadim cleanup         # reap finished, expired routine workbenches now
moadim cleanup --json  # same, as a machine-readable JSON object
moadim trigger <id>    # trigger a routine to run now, outside its schedule
moadim restart         # stop a running server (if any) and start a fresh one
moadim stop            # ask a running server to stop
moadim stop --json     # same, as a machine-readable JSON object
```

| Command            | Mode          | Behaviour |
|--------------------|---------------|-----------|
| `moadim`           | background    | Spawns a detached server, writes its PID to `~/.config/moadim/moadim.pid`, logs to `~/.config/moadim/daemon.log`, and exits. Refuses to start if one is already running. |
| `moadim -i`        | interactive   | Runs in the foreground; logs to the terminal; Ctrl-C stops it. |
| `moadim restart`   | background    | Stops the running server (if any) and spawns a fresh detached instance, so you get a clean process without a separate stop/start. Prints the PID rotation as `restarted: pid <old> -> <new>` (old reads `none` when nothing was running) so scripts/logs can confirm the process actually changed. |
| `moadim stop`      | —             | Sends `POST /shutdown` to the running server for a graceful stop. Add `--json` for `{"running":bool,"pid":N\|null,"address":"127.0.0.1:5784"}` (the `pid` is read before the shutdown request, since a graceful stop clears the pid file). Exits `0` when a running server was asked to shut down, `3` when none was reachable. |
| `moadim status`    | —             | Prints whether a server is reachable on `127.0.0.1:5784`. Add `--json` for `{"running":bool,"pid":N\|null,"address":"127.0.0.1:5784","uptime_secs":N\|null,"version":S\|null}` — `uptime_secs`/`version` come from the server's `GET /health`, so a single call returns liveness **and** age/version (both `null` when no server answers). Exits `0` when running, `3` when not. |
| `moadim cleanup`   | —             | Sends `POST /api/v1/routines/cleanup` to the running server and prints how many finished, expired routine workbenches were reaped (the on-demand version of the hourly sweep). Add `--json` for `{"running":bool,"removed":N,"address":"127.0.0.1:5784"}` (matching `status`/`stop --json`'s shape). Exits `0` when running, `3` when not. |
| `moadim trigger <id>` | —          | Sends `POST /api/v1/routines/{id}/trigger` to the running server, launching the routine immediately outside its schedule (the terminal equivalent of the REST/MCP on-demand trigger). Prints `triggered routine <id>` on success. Exits `0` when triggered, `3` when no server is reachable, and `1` with `no routine with id <id>` on a `404`. (`moadim run <id>` is kept as a hidden back-compat alias.) |

`status`, `cleanup`, and `stop` follow a script-friendly exit-code contract so callers can branch
on `$?` without parsing stdout: they exit `0` when a server is running (and `cleanup` swept, `stop`
asked it to shut down) and `3` when no server is reachable. Any other failure exits non-zero (`1`)
with a message on stderr.

### Data commands

Beyond lifecycle, the CLI exposes **every** routine action the REST API and MCP tools
do — they are thin clients that send the same JSON to the running server and print its response
(pretty-printed JSON, or raw text for logs / the iCalendar feed). Like `status`/`stop`/`cleanup`,
they exit `3` when no server is reachable and `1` on a non-2xx response.

```sh
# Routines (alias: `routine`)
moadim routines create --schedule "0 8 * * *" --title "Daily" --agent claude --prompt "..." \
  --repositories '[{"repository":"https://github.com/me/repo","branch":"main"}]'
moadim routines list
moadim routines get <id>
moadim routines update <id> --title "Renamed" --ttl-secs 3600
moadim routines replace <id> --schedule "0 8 * * *" --title "Daily" --agent claude --prompt "..."
moadim routines trigger <id>
moadim routines logs <id>
moadim routines ical          # iCalendar feed of upcoming fire times
moadim routines delete <id>

# Misc
moadim agents                 # list available agent keys
moadim echo "hello"           # echo via the server (with a server timestamp)
```

Pass `--help` to any subcommand (e.g. `moadim routines create --help`) for the full flag list.
`--repositories` (routines) takes raw JSON. Optional flags map to a PATCH so
only what you pass changes; `create`/`replace` send the full object.

### Scripting

`status`, `cleanup`, and `stop` each accept `--json` for a single-line, machine-readable object
on stdout. Paired with the exit codes above, a caller gets the full contract without parsing prose:

| Command            | `--json` shape | Exit codes |
|--------------------|----------------|------------|
| `moadim status --json`  | `{"running":bool,"pid":N\|null,"address":"127.0.0.1:5784","uptime_secs":N\|null,"version":S\|null}` — `pid` is `null` when no pid file is present; `uptime_secs`/`version` are folded in from the server's `GET /health` and are `null` when no server answers | `0` running, `3` not |
| `moadim cleanup --json` | `{"running":bool,"removed":N,"address":"127.0.0.1:5784"}` — `removed` is `0` when no server is running; `address` is the bound endpoint (matching `status`/`stop --json`) | `0` running, `3` not |
| `moadim stop --json`    | `{"running":bool,"pid":N\|null,"address":"127.0.0.1:5784"}` — `running` is `true` when a running server was asked to shut down; `pid` is the stopped server's PID (read before shutdown) or `null` when none was reachable | `0` running, `3` not |

Any other failure exits `1` with a message on stderr. The object is always a single line, so
`moadim status --json | jq -r .pid` and similar pipelines work without buffering.

Putting the contract to use — branch on the exit code, then read the JSON only when you need a field:

```sh
# Start the server only if one isn't already running (status exits 3 when not).
if ! moadim status --json >/dev/null; then
  moadim
fi

# Grab the running server's PID for a downstream check (empty when not running).
pid=$(moadim status --json | jq -r '.pid // empty')

# Reap expired routine workbenches and report how many were freed.
removed=$(moadim cleanup --json | jq -r .removed)
echo "moadim: reaped ${removed} workbench(es)"
```

Because the default mode is detached, you stop the server **from the client**:
press the **STOP** button in the UI header, run `moadim stop`, or send
`POST /shutdown`. (During development, `cargo run -- --interactive` keeps it in
the foreground.)

Starts on `http://127.0.0.1:5784`. On startup the server loads all routines,
seeds any missing built-in default routines, and rewrites the **routines**
crontab block — so a block that went stale while the server was stopped (e.g.
emptied by an earlier run) is regenerated and scheduled routines keep firing.
Reverse sync (crontab → moadim) is not run, so manual edits inside the managed
block are never imported — they are overwritten by the next forward sync.

### Bind address

The server binds to `127.0.0.1:5784` by default — the same address every client command
(`status`, `stop`, `cleanup`, …) probes. Override it with the `MOADIM_BIND_ADDR` environment
variable, set identically for the server and any client you run against it:

```sh
# Bind the server to a different port (still on loopback)…
MOADIM_BIND_ADDR=127.0.0.1:7000 moadim

# …and point client commands at the same address.
MOADIM_BIND_ADDR=127.0.0.1:7000 moadim status
```

> **⚠️ Keep the bind address on loopback.** The REST API and the MCP endpoint are
> **unauthenticated** — there is no token, password, or per-client check. Anyone who
> can reach the bind address can create, edit, and trigger routines, and a triggered
> routine launches an agent on your machine with your credentials. Binding to a
> routable interface therefore hands that control surface — effectively remote code
> execution — to the network: a LAN address exposes it to everyone on the subnet, and
> `0.0.0.0` exposes it to every interface, including the public internet if the host is
> reachable. The default `127.0.0.1` keeps the daemon private to the local machine;
> leave it there. If you genuinely need remote access, put the daemon behind an
> authenticating reverse proxy (or a firewall / VPN / SSH tunnel) instead of widening
> `MOADIM_BIND_ADDR`.

Because the override changes both the bind and the probe target, a client started without it
keeps looking at the default `127.0.0.1:5784` and will report the relocated server as not running.
Export the variable in your shell profile to make the change stick across commands. All the
`127.0.0.1:5784` addresses shown above and in the `--json` payloads reflect the default; they
follow `MOADIM_BIND_ADDR` when it is set.

### Log format

The server logs to stdout (foreground) or `~/.config/moadim/daemon.log` (background) using
`env_logger`'s default human-readable format. Set `MOADIM_LOG_FORMAT=json` to switch to one JSON
object per line instead, for shipping `daemon.log` into a log aggregator (Loki, ELK, Vector,
CloudWatch, `jq`-based tooling):

```sh
MOADIM_LOG_FORMAT=json moadim -i
```

Each line carries `ts` (RFC 3339), `level`, `target`, and `msg`:

```json
{"ts":"2026-07-01T08:10:34.700305+00:00","level":"INFO","target":"moadim::routines::service","msg":"..."}
```

An unset or unrecognized value falls back to the default text format. `RUST_LOG` keeps filtering
levels the same way in both formats.

## MCP usage

> _This is where the loop closes: your agent reads, schedules, and re-fires its own jobs. Loop engineering with a daemon in the middle._

The server exposes an MCP endpoint at `http://localhost:5784/mcp`. Connect any MCP-compatible client.

### Claude Code

Add moadim at **user scope** so it's available across all your projects. moadim is a global daemon (one local server, one crontab) — there's no per-project state, so project scope would only force you to re-add it in every repo.

```sh
claude mcp add --scope user --transport http moadim http://localhost:5784/mcp
```

### Any MCP client

```
transport: streamable-http
url:       http://localhost:5784/mcp
```

## API

The daemon serves an interactive **Swagger UI** at
[`http://localhost:5784/docs`](http://localhost:5784/docs) — open it in a
browser to explore every endpoint, read the schemas, and try calls against your
running server directly. The raw OpenAPI spec is at
`http://localhost:5784/docs/openapi.json`.

Both are regenerated at build time from the source; the checked-in snapshot
lives in [`apis/`](apis/).

## Changelog

Release history lives in [`CHANGELOG.md`](CHANGELOG.md), following the
[Keep a Changelog](https://keepachangelog.com/) format.

## License

Licensed under the [MIT License](LICENSE).
