# moadim

[![crates.io](https://img.shields.io/crates/v/moadim.svg)](https://crates.io/crates/moadim)
[![docs.rs](https://img.shields.io/docsrs/moadim)](https://docs.rs/moadim)
[![CI](https://img.shields.io/github/actions/workflow/status/moadim-io/daemon/test.yml?branch=main&label=CI)](https://github.com/moadim-io/daemon/actions/workflows/test.yml)
[![License: MIT](https://img.shields.io/crates/l/moadim.svg)](https://opensource.org/licenses/MIT)

> **Loop engineering, on a schedule.** Stop prompting your agents — design the loop that prompts them.
>
> **Cron jobs that run while you sleep.** One port. Three interfaces. Zero drift.
>
> _Set the loop. Forget the keyboard. moadim fires the prompt so you don't have to._

**One-line install** — install Rust/Cargo, install moadim, then run it:

```sh
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y && . "$HOME/.cargo/env" && cargo install --locked moadim && moadim
```

Rust server that schedules **cron jobs** (run a script) and **routines** (run an
AI agent), exposing both over three interfaces simultaneously:

- **UI** (`http://localhost:5784/`) — browser dashboard for managing jobs and routines
- **REST** (`http://localhost:5784/api/v1`) — standard HTTP API for browsers, CLI tools, and services
- **MCP** (`http://localhost:5784/mcp`) — [Model Context Protocol](https://modelcontextprotocol.io) for AI agents (Claude, etc.)

All three share the same port. Jobs and routines created through any interface are
automatically synced to the OS crontab so they actually run on schedule. See
[Routines](#routines) for the agent-loop engine.

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

## Features

> _Close the loop. Skip the keyboard. Loop engineering, shipped as a daemon._

- Jobs created via REST or MCP are written into your OS crontab automatically
- Job declarations live in `~/.config/moadim/jobs/` — git-trackable, diff-friendly
- Handlers are executable scripts in `~/.config/moadim/handlers/` — any language, also git-trackable
- **Routines** schedule an AI agent instead of a script — a prompt + schedule + agent, stored in `~/.config/moadim/routines/` (see [Routines](#routines))
- **Agents** are a registry of coding agents (`claude`, …) under `~/.config/moadim/agents/<name>.toml`, referenced by routines
- Each routine run executes in a throwaway **workbench** under `~/.config/moadim/workbenches/`, reaped on an hourly cleanup sweep
- `job.local.toml` per job for secrets and machine-specific overrides that stay off-git
- Same REST and MCP interface — no logic duplication between protocols
- API spec auto-generated at build time into `apis/`

## Directory layout

```
~/.config/moadim/
├── jobs/
│   ├── daily-report/
│   │   ├── job.toml        # tracked — commit this
│   │   ├── job.local.toml  # untracked — local overrides (secrets, machine-specific config)
│   │   └── job.local.log         # untracked — runtime log
│   ├── cleanup-temp/
│   │   ├── job.toml
│   │   └── job.local.log
│   └── sync-calendar/
│       ├── job.toml
│       └── job.local.toml
├── handlers/
│   ├── send-report.sh
│   ├── cleanup-temp.py
│   └── sync-calendar.sh
├── routines/                  # scheduled AI-agent tasks (see ## Routines)
│   └── nightly-triage/
│       ├── routine.toml       # tracked — schedule, agent, prompt, repositories
│       ├── prompt.md          # tracked — the rendered prompt handed to the agent
│       ├── run.sh             # generated — the crontab entry invokes this
│       └── .gitignore         # generated — excludes *.local.* and *.log
├── agents/                    # registered coding agents referenced by routines
│   └── claude.toml
└── workbenches/               # per-run throwaway dirs, reaped on the hourly sweep
```

## Crontab sync

> _Your crontab, your rules — moadim keeps its own block in sync._

Moadim owns a single block inside your crontab. Everything outside that block is untouched.

```
# BEGIN MOADIM
# Managed by moadim — edits here are overwritten on the next sync
30 9 * * 1-5 /home/user/.config/moadim/handlers/send-report # moadim:uuid
0 0 * * 0 /home/user/.config/moadim/handlers/cleanup-temp # moadim:uuid
# END MOADIM
```

**Forward sync (moadim → crontab):** any time you create, update, or delete a job via the UI, REST, or MCP, the crontab block is rewritten immediately. Disabled jobs are excluded from the block.

**Reverse sync (crontab → moadim) is not currently enabled.** Edit jobs through the UI, REST, or MCP rather than by hand: manual changes inside the block do **not** sync back into moadim and are overwritten by the next forward sync. (The reverse-sync parser exists but is not wired to run — tracked in [#218](https://github.com/moadim-io/daemon/issues/218).)

**Schedule format:** standard 5-field cron (`min hour dom month dow`), same as the OS crontab. `@keyword` shortcuts (`@hourly`, `@daily`, `@weekly`, `@monthly`, `@yearly`, `@annually`) are also accepted. `@reboot` and `@midnight` are **not** supported via the API and are rejected with `400 Bad Request`.

**Timezone:** because jobs run via the OS crontab, schedules are evaluated in the host's **local system timezone**, not UTC. A schedule of `0 9 * * *` fires at 09:00 local time. AI agents in particular should not pre-convert times to UTC.

## Handlers

Handlers are executable scripts under `~/.config/moadim/handlers/`. The `handler` field in `job.toml` is the filename without extension.

```
handlers/send-report.sh      ← handler = "send-report"
handlers/cleanup-temp.py     ← handler = "cleanup-temp"
```

Any executable works — shell, Python, Node, compiled binary. The server passes job metadata as environment variables prefixed with `MOADIM_`.

```sh
#!/usr/bin/env bash
# ~/.config/moadim/handlers/send-report.sh

curl -s -X POST "https://api.example.com/report" \
  -H "Authorization: Bearer $MOADIM_API_KEY" \
  -d "recipient=$MOADIM_RECIPIENT"
```

Multiple jobs can share one handler, differing only in schedule or metadata:

```
jobs/daily-report/job.toml   → handler = "send-report"
jobs/weekly-digest/job.toml  → handler = "send-report"
```

Handlers are git-trackable alongside jobs:

```sh
cd ~/.config/moadim
git add jobs/ handlers/
git commit -m "initial jobs and handlers"
```

## Job declarations

Each job is a folder under `~/.config/moadim/jobs/`. The folder name is the job ID.

Each job folder contains an auto-generated `.gitignore` that excludes `*.local.*` and `*.log` files — no manual setup needed.

### `job.toml`

Tracked configuration — schedule, handler, and shared metadata.

```toml
# ~/.config/moadim/jobs/daily-report/job.toml

schedule = "30 9 * * 1-5"   # cron expression (min hour dom month dow)
handler  = "send-report"     # filename in ~/.config/moadim/handlers/ (no extension)
enabled  = true              # omit to default to true

[metadata]
recipient = "team@example.com"
timezone  = "Asia/Jerusalem"
```

| Field        | Type   | Required | Description                                                            |
| ------------ | ------ | -------- | ---------------------------------------------------------------------- |
| `schedule`   | string | yes      | Cron expression: `min hour dom month dow` or `@hourly`/`@daily`/`@weekly`/`@monthly`/`@yearly`/`@annually`. |
| `handler`    | string | yes      | Script name in `handlers/` (without extension)                         |
| `enabled`    | bool   | no       | Defaults to `true`. Set `false` to pause without deleting.             |
| `[metadata]` | table  | no       | Key/value pairs passed to the handler as `MOADIM_*` env vars.          |

### `job.local.toml`

Untracked overrides — machine-specific values or secrets that should not be committed. Loaded after `job.toml`; local values win on any conflict.

```toml
# ~/.config/moadim/jobs/daily-report/job.local.toml

enabled = false           # overrides job.toml enabled = true → job is paused locally

[metadata]
api_key = "sk-..."        # secret — never commit
recipient = "me@local"    # overrides job.toml recipient
```

### `job.local.log`

Append-only log written by the server on each run. Gitignored via `*.local.*`. Readable in the UI via the LOGS button or `GET /api/v1/cron-jobs/{id}/logs`.

```
2026-06-11T09:30:00Z [daily-report] run started
2026-06-11T09:30:01Z [daily-report] run finished OK (1.2s)
```

## Routines

> _Cron jobs run a script. Routines run an agent._

A **routine** is a scheduled AI-agent task — the agent-driven sibling of a cron
job. Where a job fires a handler script, a routine fires a prompt at a coding
agent (e.g. Claude) on a cron schedule, each run inside its own throwaway
workbench.

Routines are stored as folders under `~/.config/moadim/routines/<id>/`,
git-trackable just like jobs:

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
`trigger_routine`, and `cleanup_routines`.

**Agents:** the `agent` field resolves to a config at
`~/.config/moadim/agents/<agent>.toml`. API responses include
`agent_registered` so callers can tell whether the named agent is configured on
the host.

## Running

Moadim runs as a local daemon. By default it starts **in the background**:

```sh
moadim                 # start detached, print the PID, return to the shell
moadim --interactive   # run in the foreground, attached to the terminal (Ctrl-C to stop)
moadim status          # report whether a server is running
moadim status --json   # same, as a machine-readable JSON object
moadim cleanup         # reap finished, expired routine workbenches now
moadim cleanup --json  # same, as a machine-readable JSON object
moadim restart         # stop a running server (if any) and start a fresh one
moadim stop            # ask a running server to stop
moadim stop --json     # same, as a machine-readable JSON object
```

| Command            | Mode          | Behaviour |
|--------------------|---------------|-----------|
| `moadim`           | background    | Spawns a detached server, writes its PID to `~/.config/moadim/moadim.pid`, logs to `~/.config/moadim/daemon.log`, and exits. Refuses to start if one is already running. |
| `moadim -i`        | interactive   | Runs in the foreground; logs to the terminal; Ctrl-C stops it. |
| `moadim restart`   | background    | Stops the running server (if any) and spawns a fresh detached instance, so you get a clean process without a separate stop/start. Prints the PID rotation as `restarted: pid <old> -> <new>` (old reads `none` when nothing was running) so scripts/logs can confirm the process actually changed. |
| `moadim stop`      | —             | Sends `POST /shutdown` to the running server for a graceful stop. Add `--json` for `{"running":bool,"pid":N\|null,"address":"127.0.0.1:5784"}` (matching `status --json`'s shape; the `pid` is read before the shutdown request, since a graceful stop clears the pid file). Exits `0` when a running server was asked to shut down, `3` when none was reachable. |
| `moadim status`    | —             | Prints whether a server is reachable on `127.0.0.1:5784`. Add `--json` for `{"running":bool,"pid":N\|null,"address":"127.0.0.1:5784"}`. Exits `0` when running, `3` when not. |
| `moadim cleanup`   | —             | Sends `POST /api/v1/routines/cleanup` to the running server and prints how many finished, expired routine workbenches were reaped (the on-demand version of the hourly sweep). Add `--json` for `{"running":bool,"removed":N}`. Exits `0` when running, `3` when not. |

`status`, `cleanup`, and `stop` follow a script-friendly exit-code contract so callers can branch
on `$?` without parsing stdout: they exit `0` when a server is running (and `cleanup` swept, `stop`
asked it to shut down) and `3` when no server is reachable. Any other failure exits non-zero (`1`)
with a message on stderr.

### Scripting

`status`, `cleanup`, and `stop` each accept `--json` for a single-line, machine-readable object
on stdout. Paired with the exit codes above, a caller gets the full contract without parsing prose:

| Command            | `--json` shape | Exit codes |
|--------------------|----------------|------------|
| `moadim status --json`  | `{"running":bool,"pid":N\|null,"address":"127.0.0.1:5784"}` — `pid` is `null` when no pid file is present | `0` running, `3` not |
| `moadim cleanup --json` | `{"running":bool,"removed":N}` — `removed` is `0` when no server is running | `0` running, `3` not |
| `moadim stop --json`    | `{"running":bool,"pid":N\|null,"address":"127.0.0.1:5784"}` — same shape as `status --json`; `running` is `true` when a running server was asked to shut down; `pid` is the stopped server's PID (read before shutdown) or `null` when none was reachable | `0` running, `3` not |

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

Starts on `http://127.0.0.1:5784`. On startup the server:

1. Loads all jobs from `~/.config/moadim/jobs/`.
2. Reads your crontab and applies any changes made to the moadim block while the server was stopped.
3. Writes all enabled managed jobs back into the crontab block.

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

Full interface definitions are auto-generated at build time — see the [`apis/`](apis/) folder.

## Changelog

Release history lives in [`CHANGELOG.md`](CHANGELOG.md), following the
[Keep a Changelog](https://keepachangelog.com/) format.

## License

Licensed under the [MIT License](LICENSE).
