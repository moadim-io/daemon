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

The daemon reports whether `tmux` and `python3` resolve on its `PATH` in
`GET /api/v1/health` (under `dependencies`) and logs a warning at startup when
either is missing, so a misconfigured host is easy to spot. See the built-in
`claude` agent's prerequisites below for why `python3` matters.

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
- Each routine run executes in a throwaway **workbench** under `~/.moadim/workbenches/` (a separate tree from the config dir), reaped on a periodic (5-minute) cleanup sweep
- Same REST and MCP interface — no logic duplication between protocols
- API spec auto-generated at build time into `apis/`

## Directory layout

```
~/.config/moadim/
├── routines/                  # scheduled AI-agent tasks (see ## Routines)
│   └── nightly-triage/
│       ├── routine.toml       # tracked — metadata (agent, repositories, [env], …)
│       ├── schedule.cron      # tracked — one cron entry
│       ├── routine.local.toml # gitignored, optional — secret/local env var overrides
│       ├── prompts/
│       │   ├── prompt.pure.md      # tracked — the raw, user-authored prompt
│       │   └── prompt.compiled.local.md  # gitignored — derived, rendered prompt
│       └── .gitignore         # generated — excludes *.compiled.*, *.local.*, and *.log
├── agents/                    # registered coding agents referenced by routines
│   └── claude.toml
└── user_prompt.md             # optional — appended to every routine's prompt (see ## Routines)

~/.moadim/                     # runtime tree, separate from the config dir above
└── workbenches/               # per-run throwaway dirs, reaped on the periodic sweep
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
workbench. The schedule lives in a sibling `schedule.cron` file; the rest of
the routine metadata stays in `routine.toml`.

Routines are stored as folders under `~/.config/moadim/routines/<id>/`,
git-trackable:

```
~/.config/moadim/routines/
└── nightly-triage/
    ├── routine.toml       # tracked — metadata (agent, repositories, [env], …)
    ├── schedule.cron      # tracked — one cron entry
    ├── routine.local.toml # gitignored, optional — secret/local env var overrides
    ├── prompts/
    │   ├── prompt.pure.md      # tracked — the raw, user-authored prompt
    │   └── prompt.compiled.local.md  # gitignored — derived, rendered prompt
    └── .gitignore     # generated — excludes *.compiled.*, *.local.*, and *.log
```

| Field          | Type   | Required | Description                                                                                  |
| -------------- | ------ | -------- | -------------------------------------------------------------------------------------------- |
| `schedule`     | string | yes      | Cron expression (`min hour dom month dow` or `@daily`, …), stored in `schedule.cron` and evaluated in the host's local timezone — **not** UTC. |
| `title`        | string | yes      | Human name; slugified to name the run workbench and tmux session.                            |
| `agent`        | string | yes      | Agent registry key (e.g. `claude`), resolved from `~/.config/moadim/agents/<agent>.toml`.    |
| `model`        | string | no       | Model ID to run the agent with (e.g. `claude-sonnet-4-6`), passed as `--model` on the agent invocation. `None`/omitted uses the agent's own default. |
| `goal`         | string | no       | A very short (≤5 lines) statement of the routine's goal — the "why" behind the prompt. Rendered into `prompt.md` as a `## Goal` preamble. |
| `repositories` | list   | no       | Git repos listed in the prompt as context. Moadim does **not** clone them — the agent does.   |
| `machines`     | list   | no       | Machine identities this routine runs on (matched against this install's resolved machine name — see below). Defaults to empty — **an empty list runs nowhere**, so a new routine is dormant until explicitly assigned. |
| `enabled`      | bool   | no       | Defaults to `true`. Set `false` to pause without deleting.                                    |
| `ttl_secs`     | int    | no       | How long a finished run's workbench is retained before auto-cleanup. Caps the cron-derived retention lower — it can only shorten, never extend it. `None` uses the cron-derived value. |
| `max_runtime_secs` | int | no       | Max wall-clock seconds a single run may execute before the cleanup watchdog force-kills its (hung) tmux session; the workbench is then reaped under the normal TTL rules. Caps the cron-derived runtime (`min(MAX_RUNTIME_SECS, cron interval)`) lower — it can only shorten, never extend it. `None` uses the cron-derived value. |
| `tags`         | list   | no       | Free-form labels for grouping/filtering routines (e.g. `"nightly"`). Defaults to empty; each entry is trimmed and must be non-blank. |
| `env`          | map    | no       | Environment variables injected into the agent's shell session at launch (see [Environment variables](#environment-variables) below). Defaults to empty. |

### Environment variables

By default every routine's agent inherits the daemon operator's entire login-shell environment
(`~/.profile` is sourced before launch), so all routines on a host share one `GH_TOKEN` and one set
of credentials. The `env` field lets a routine declare its own scoped variables instead:

- **`routine.toml`'s `env` table** — a plain `string → string` map, tracked and git-committed.
  Use it for non-secret config (a model override the agent reads, a base URL, a feature flag) —
  anything you're fine with landing in the config repo's history.

  ```toml
  [env]
  GITHUB_ORG = "my-other-org"
  FEATURE_FLAG = "beta"
  ```

- **`routine.local.toml`** — an untracked sibling file next to `routine.toml`, gitignored by the
  same `*.local.*` pattern that already covers `state.local.toml` and
  `prompt.compiled.local.md`. Use it for secrets (a scoped `GH_TOKEN`, an API key) that must never
  be committed. Same `[env]` table shape; its keys **win** over `routine.toml`'s when both set the
  same name.

  ```toml
  # ~/.config/moadim/routines/nightly-triage/routine.local.toml — never committed
  [env]
  GH_TOKEN = "ghp_..."
  ```

Both sources are merged (local wins on conflict) and emitted as `export KEY=value` statements right
after the curated `PATH` export and before the agent launches, so they override any
profile-inherited value for that run only — the daemon's own environment, and every other routine's,
are untouched.

**Validation:** keys must be POSIX-portable shell identifiers (`[A-Za-z_][A-Za-z0-9_]*`); values
must not contain newlines. `routine.toml`'s `env` table is validated on create/update (REST/MCP) and
rejected outright if invalid — nothing bad reaches the crontab. `routine.local.toml` is a file you
edit by hand, so it isn't validated over the API; a malformed entry there is simply skipped (with a
warning in the daemon log) rather than breaking the launch.

**Redaction:** env var *values* — from either source — never appear in a `GET`/`POST` routine
response, the UI, or any log line. A routine response instead carries `env_keys`: the sorted, deduplicated
list of every configured key name (from both `routine.toml` and `routine.local.toml`), so a client
can show *what's* set without ever seeing a value.

**Machine identity:** used to filter which of a routine's `machines` entries apply to *this*
install when several daemons share one `~/.config/moadim` config repo (a laptop, a work box, a
server). Resolved in priority order: the `MOADIM_MACHINE` env var (trimmed, non-empty), then the
`name` field in the gitignored `~/.config/moadim/machine.local.toml`, then the system hostname
(auto-generated into that file on first run). Inspect or change it with `moadim machine show` /
`moadim machine set <name>` (see "Misc" below).

**Workbenches and cleanup:** each run executes in a workbench under
`~/.moadim/workbenches/`. Finished, expired workbenches are reaped on a
periodic (5-minute) sweep so they don't accumulate; trigger a sweep on demand
with `moadim cleanup`. Sessions still running are never reaped.

TTL is time-only, so a handful of concurrent large runs (e.g. big repo clones)
can pile up disk before any TTL elapses. Set `MOADIM_MAX_WORKBENCH_DISK_BYTES`
to a total byte ceiling for the whole `~/.moadim/workbenches/` tree; once
exceeded, the sweep also evicts finished workbenches oldest-first (regardless
of their individual TTL) until back under it. A live session is never evicted.
Unset or `0` (the default) keeps today's unbounded-by-size behavior.

A routine already refuses to overlap with its own still-running fire, but
nothing on its own bounds how many *different* routines run at once — the OS
crontab naturally aligns fires from separate routines onto the same minute
boundary (e.g. `*/5 * * * *`), so a shared tick can otherwise launch an
unbounded thundering herd of agent sessions. Set `MOADIM_MAX_CONCURRENT_RUNS`
to cap how many routine agent sessions may be alive at once. Unset or `0`
(the default) means unlimited; once the cap is reached, a new fire is
skipped — logging the reason — rather than launched, and is picked up again
on its next scheduled tick. The count is derived from actual live tmux
sessions, not an in-memory counter, so it stays correct across a daemon
crash/restart. The cap can also be set per-machine (persisted in
`machine.local.toml`, editable from the UI/REST settings) without touching
the environment; `MOADIM_MAX_CONCURRENT_RUNS` takes precedence over that
override when both are set.

**REST** — under the `/api/v1` prefix:

```
GET    /health                 # liveness + uptime/version, used by status/--wait and the UI health badge
GET    /metrics                # Prometheus text-exposition metrics: run counts/failures/durations, active sessions, workbench disk usage
POST   /shutdown               # graceful stop, used by `moadim stop` and the UI STOP button
POST   /restart                # stop this server and start a fresh instance, used by `moadim restart`
GET    /routines              # list (filter by ?repository=, sort by ?sort=/&order=)
POST   /routines              # create
GET    /routines/{id}         # fetch one
PUT    /routines/{id}         # replace
PATCH  /routines/{id}         # update fields
DELETE /routines/{id}         # delete
POST   /routines/{id}/trigger # run now, outside the schedule
POST   /routines/{id}/scheduled-trigger # daemon-side endpoint the generated crontab line invokes
GET    /routines/{id}/prompt-preview # composed prompt body a run would receive, no run
GET    /routines/{id}/logs    # newest workbench's agent.log as plain text
POST   /routines/cleanup      # reap expired workbenches now
GET    /routines/runs          # most recent runs across every routine, newest first (?limit=)
GET    /routines/{id}/runs     # every run workbench for one routine, newest first
GET    /routines/{id}/runs/{workbench}/log     # one specific run's agent.log
GET    /routines/{id}/runs/{workbench}/summary # one specific run's agent-authored summary.md
GET    /routines/{id}/flags    # list open flags raised against a routine
POST   /routines/{id}/flags    # raise a new flag
DELETE /routines/{id}/flags/{filename} # resolve (delete) a flag
GET    /routines/lock          # current global lock status
POST   /routines/lock          # create a lock sentinel, halting all scheduling/triggers
DELETE /routines/lock          # remove lock sentinel(s), restoring scheduling
GET    /agents                # list registered agents
GET    /routines.ics          # subscribe to fire times as a calendar feed
GET    /machine                # this machine's resolved identity
PUT    /machine                # rename this machine's identity
GET    /machines               # known machine names (union of routines' machines[] targets)
GET    /config/user-prompt     # persistent system prompt appended to every routine
PUT    /config/user-prompt     # replace it
GET    /config/max-concurrent-runs # global routine concurrency cap
PUT    /config/max-concurrent-runs # set/clear the persisted override
```

**MCP** — the same operations are exposed as tools: `list_routines`,
`get_routine`, `preview_routine_prompt`, `create_routine`, `update_routine`,
`delete_routine`, `trigger_routine`, `snooze_routine`, `set_power_saving`,
`routine_logs`, `list_routine_runs`, `create_flag`, `list_flags`,
`resolve_flag`, `list_agents`, `cleanup_workbenches`, `get_lock_status`,
`lock_routines`, `unlock_routines`, plus server-control tools `health`,
`shutdown`, and `restart`.

**Agents:** the `agent` field resolves to a config at
`~/.config/moadim/agents/<agent>.toml`. API responses include
`agent_registered` so callers can tell whether the named agent is configured on
the host.

**Built-in `claude` agent prerequisites:** the default `claude` agent needs two
things on the host beyond the `claude` CLI itself:

- **`python3`** — the agent's `setup` step runs a short `python3` snippet to
  pre-seed per-workbench state in `~/.claude.json` (trust dialog + MCP-server
  approvals) so the unattended session never blocks on a prompt. If `python3`
  is not on `PATH`, the setup step fails and the run no-ops. This is now
  surfaced (not just silent): the daemon logs a startup warning and
  `GET /api/v1/health`'s `dependencies.python3` flag reports `false`, though
  the affected routine's own health dot still shows green.
- **`tmux`** — every routine run is launched inside a tmux session (named after
  the run's workbench), so a tmux binary must be installed.

Both are present by default on most developer machines; install them explicitly
on a minimal host (e.g. a CI runner or fresh container) before relying on the
built-in `claude` agent.

### Agent configuration

Each agent is a single TOML file at `~/.config/moadim/agents/<name>.toml`, where
`<name>` is the registry key a routine's `agent` field references (the filename
stem, e.g. `claude.toml` → `claude`). On startup the daemon seeds the built-in
defaults (`claude`, `codex`, `hermes`, `pi`) into this directory **only if the file is
absent** — your edits are never overwritten — so you can both tweak a default and
register a brand-new agent by dropping in another `<name>.toml`.

| Field     | Type           | Required | Description                                                                                                                   |
| --------- | -------------- | -------- | ----------------------------------------------------------------------------------------------------------------------------- |
| `command` | string         | yes      | Executable to run (resolved on `PATH`), e.g. `"claude"`.                                                                       |
| `args`    | array<string>  | no       | Arguments passed to `command`. Supports the placeholders below. Defaults to empty.                                            |
| `instructions_file` | string | no    | Filename, relative to the workbench, that this agent reads its project instructions from — where the moadim-managed system prompt and routine-origin disclosure are written. Defaults to `CLAUDE.md` (Claude Code's convention); the built-in `codex` agent sets it to `AGENTS.md`. |
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

Examples — the headless `codex`/`hermes`/`pi` form, and the interactive `claude` form
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

```toml
# ~/.config/moadim/agents/pi.toml
command = "pi"
args = ["--approve", "-p", "@{prompt_file}"]
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
moadim status --wait   # poll until a server answers (or 30s elapse) instead of checking once
moadim cleanup         # reap finished, expired routine workbenches now
moadim cleanup --json  # same, as a machine-readable JSON object
moadim trigger <id>    # trigger a routine to run now, outside its schedule
moadim logs <id>       # print a routine's newest run log (agent.log) to stdout
moadim install         # register moadim as an OS service (launchd / systemd user)
moadim uninstall       # remove the OS service registration and the managed crontab block
moadim restart         # stop a running server (if any) and start a fresh one
moadim restart --quiet # same, printing only the `restarted: pid <old> -> <new>` line
moadim restart -i      # same, but bring the fresh instance up in the foreground
moadim restart --json  # same, as a machine-readable JSON object
moadim stop            # ask a running server to stop
moadim stop --json     # same, as a machine-readable JSON object
```

| Command            | Mode          | Behaviour |
|--------------------|---------------|-----------|
| `moadim`           | background    | Spawns a detached server, writes its PID to `~/.config/moadim/moadim.pid`, logs to `~/.config/moadim/daemon.log`, and exits. Refuses to start if one is already running. |
| `moadim -i`        | interactive   | Runs in the foreground; logs to the terminal; Ctrl-C stops it. |
| `moadim restart`   | background    | Stops the running server (if any) and spawns a fresh detached instance, so you get a clean process without a separate stop/start. Prints the PID rotation as `restarted: pid <old> -> <new>` (old reads `none` when nothing was running) so scripts/logs can confirm the process actually changed. Add `--quiet`/`-q` to print only that rotation line, suppressing the preamble and the reach/manage hint block. Add `--json` for `{"old":N\|null,"new":M,"address":"127.0.0.1:5784"}`. |
| `moadim restart -i`, `--interactive` | interactive | Stops the running server (if any), same as `moadim restart`, but brings the fresh instance up in the foreground instead of backgrounding it — mirrors `moadim -i`. |
| `moadim stop`      | —             | Sends `POST /api/v1/shutdown` to the running server for a graceful stop. Add `--json` for `{"running":bool,"pid":N\|null,"address":"127.0.0.1:5784"}` (the `pid` is read before the shutdown request, since a graceful stop clears the pid file). Exits `0` when a running server was asked to shut down, `3` when none was reachable. **Only stops the daemon process** — a routine agent already running in its own detached tmux session is independent of the daemon and keeps running (and can keep acting on your behalf) until it finishes on its own or a later `moadim` start's watchdog/cleanup sweep reaps it. |
| `moadim status`    | —             | Prints whether a server is reachable on `127.0.0.1:5784`. Add `--json` for `{"running":bool,"pid":N\|null,"address":"127.0.0.1:5784","uptime_secs":N\|null,"version":S\|null}` — `uptime_secs`/`version` come from the server's `GET /api/v1/health`, so a single call returns liveness **and** age/version (both `null` when no server answers). Add `--wait[=SECS]` to poll `GET /api/v1/health` every 200ms until it answers or `SECS` elapse (default 30) instead of checking once, so a launch script can block on startup rather than sleeping blindly. Exits `0` when running, `3` when not (including a `--wait` timeout). |
| `moadim cleanup`   | —             | Sends `POST /api/v1/routines/cleanup` to the running server and prints how many finished, expired routine workbenches were reaped and the disk space freed, e.g. `cleanup removed 3 workbenches (freed 12.4 MB)` (the on-demand version of the periodic sweep). Add `--json` for `{"running":bool,"removed":N,"freed_bytes":N,"address":"127.0.0.1:5784"}` (matching `status`/`stop --json`'s shape). Exits `0` when running, `3` when not. |
| `moadim trigger <id>` | —          | Sends `POST /api/v1/routines/{id}/trigger` to the running server, launching the routine immediately outside its schedule (the terminal equivalent of the REST/MCP on-demand trigger). Prints `triggered routine <id>` on success. Exits `0` when triggered, `3` when no server is reachable, and `1` with `no routine with id <id>` on a `404`. (`moadim run <id>` is kept as a hidden back-compat alias.) |

`status`, `cleanup`, and `stop` follow a script-friendly exit-code contract so callers can branch
on `$?` without parsing stdout: they exit `0` when a server is running (and `cleanup` swept, `stop`
asked it to shut down) and `3` when no server is reachable. Any other failure exits non-zero (`1`)
with a message on stderr.

**Stop under a service install:** when moadim is installed as an OS service (`moadim install` — a
systemd user unit on Linux, a launchd agent on macOS), `moadim stop` makes the daemon **stay
stopped**. The supervisor restarts only on a *failure* exit (systemd `Restart=on-failure`, launchd
`KeepAlive = { SuccessfulExit = false }`), so a clean shutdown — `moadim stop`, the UI STOP button,
`POST /api/v1/shutdown`, all of which exit `0` — is not resurrected, while a crash is still auto-restarted.
To start the service again after a stop, use `moadim` (or your supervisor's `systemctl --user start`
/ `launchctl` controls).

### Shell completions

`moadim completions <shell>` prints a completion script for `bash`, `zsh`, `fish`, `powershell`, or
`elvish` to stdout; redirect it to wherever your shell loads completions from:

```sh
# bash (adjust the path to wherever your bash-completion install looks)
moadim completions bash > /etc/bash_completion.d/moadim

# zsh (any directory on $fpath; start a new shell, or `compinit`, afterward)
moadim completions zsh > "${fpath[1]}/_moadim"

# fish
moadim completions fish > ~/.config/fish/completions/moadim.fish
```

### Data commands

Beyond lifecycle, the CLI exposes the same routine actions the REST API and MCP tools
do — they are thin clients that send the same JSON to the running server and print its response
(pretty-printed JSON, or raw text for logs / the iCalendar feed). Like `status`/`stop`/`cleanup`,
they exit `3` when no server is reachable and `1` on a non-2xx response.

Routine flags (`create_flag`/`list_flags`/`resolve_flag`) and the global routine lock
(`get_lock_status`/`lock_routines`/`unlock_routines`) are REST/MCP-only for now — there is no
`moadim` subcommand for them yet.

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

# Pause / resume a single routine (id or slug) without editing its definition
moadim enable <routine>       # set enabled = true
moadim disable <routine>      # set enabled = false  (--json for a {routine,enabled} object)

# Misc
moadim agents                 # list available agent keys
moadim machine show           # print this install's resolved machine name + where it came from
moadim machine set <name>     # persist a machine name to machine.local.toml
moadim machine list           # list distinct machine names referenced by any routine's `machines`
```

Pass `--help` to any subcommand (e.g. `moadim routines create --help`) for the full flag list.
`--repositories` (routines) takes raw JSON. Optional flags map to a PATCH so
only what you pass changes; `create`/`replace` send the full object.

### Scripting

`status`, `cleanup`, and `stop` each accept `--json` for a single-line, machine-readable object
on stdout. Paired with the exit codes above, a caller gets the full contract without parsing prose:

| Command            | `--json` shape | Exit codes |
|--------------------|----------------|------------|
| `moadim status --json`  | `{"running":bool,"pid":N\|null,"address":"127.0.0.1:5784","uptime_secs":N\|null,"version":S\|null}` — `pid` is `null` when no pid file is present; `uptime_secs`/`version` are folded in from the server's `GET /api/v1/health` and are `null` when no server answers | `0` running, `3` not |
| `moadim cleanup --json` | `{"running":bool,"removed":N,"freed_bytes":N,"address":"127.0.0.1:5784"}` — `removed`/`freed_bytes` are `0` when no server is running; `address` is the bound endpoint (matching `status`/`stop --json`) | `0` running, `3` not |
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

# Block until the just-launched server answers (or 10s pass) instead of a blind sleep.
moadim
moadim status --wait=10 --json | jq -r .running

# Reap expired routine workbenches and report how many were freed.
removed=$(moadim cleanup --json | jq -r .removed)
echo "moadim: reaped ${removed} workbench(es)"
```

Because the default mode is detached, you stop the server **from the client**:
press the **STOP** button in the UI header, run `moadim stop`, or send
`POST /api/v1/shutdown`. (During development, `cargo run -- --interactive` keeps it in
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

Because that risk is easy to hit by accident (a copy-pasted `0.0.0.0` to "just get it
working" on a container/VM), the daemon **refuses to start** if `MOADIM_BIND_ADDR` resolves
to a non-loopback address, unless you explicitly opt in:

```sh
# Refused at startup — 0.0.0.0 is not loopback and MOADIM_ALLOW_REMOTE isn't set:
MOADIM_BIND_ADDR=0.0.0.0:5784 moadim
# moadim: refusing to bind to 0.0.0.0:5784: it is not loopback-only, and the REST/MCP API
# has no authentication — ... Set MOADIM_ALLOW_REMOTE=1 to start anyway if you understand
# and accept that risk.

# Starts, but logs a prominent warning every time, because you opted in:
MOADIM_ALLOW_REMOTE=1 MOADIM_BIND_ADDR=0.0.0.0:5784 moadim
```

`MOADIM_ALLOW_REMOTE` must be exactly `1` — any other value (unset, `true`, `yes`, …) is
treated as not opted in, so a typo fails closed rather than silently exposing the API. This
is a startup gate only, not authentication: once opted in, the API is exactly as open as
described above, so still prefer a reverse proxy / firewall / VPN / SSH tunnel over setting
this. See issue #253.

Because the override changes both the bind and the probe target, a client started without it
keeps looking at the default `127.0.0.1:5784` and will report the relocated server as not running.
Export the variable in your shell profile to make the change stick across commands. All the
`127.0.0.1:5784` addresses shown above and in the `--json` payloads reflect the default; they
follow `MOADIM_BIND_ADDR` when it is set.

Loopback isn't a full security boundary against a **browser**: any page you have open can still
send requests to `http://127.0.0.1:5784`, and DNS rebinding can point an attacker-controlled domain
at that same address. To close that off, every request's `Host` header is checked against an
allowlist (`localhost`, `127.0.0.1`, `[::1]`, and the configured bind address), and a state-changing
request (`POST`/`PUT`/`PATCH`/`DELETE`) carrying a cross-origin `Origin` header is rejected too — both
with `403`. Non-browser clients (`curl`, the `moadim` CLI, the MCP transport) are unaffected, since
they never send a forged `Origin` and always address the daemon by an allowlisted host. If you put
the daemon behind a reverse proxy on another hostname, add it to the allowlist with
`MOADIM_ALLOWED_HOSTS` (comma-separated `host[:port]` entries):

```sh
MOADIM_ALLOWED_HOSTS=moadim.example.internal moadim
```

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
