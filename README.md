# moadim-server

Rust server that exposes the same functionality over three protocols simultaneously:

- **REST** (`http://localhost:5784/`) — standard HTTP API for browsers, CLI tools, and services
- **GraphQL** (`http://localhost:5784/graphql`) — typed query and mutation API with interactive playground
- **MCP** (`http://localhost:5784/mcp`) — [Model Context Protocol](https://modelcontextprotocol.io) for AI agents (Claude, etc.)

All three run on the same port.

## Features

- Same cron-job, health, and echo logic reachable via REST and MCP
- Cron-job declarations live in `~/.config/moadim/jobs/` — git-trackable, diff-friendly
- Handlers are executable scripts in `~/.config/moadim/handlers/` — any language, also git-trackable
- API interfaces auto-generated at build time into `apis/`
- Static browser client served from `static/index.html`

## Directory layout

```
~/.config/moadim/
├── jobs/
│   ├── daily-report/
│   │   ├── job.toml        # tracked — commit this
│   │   ├── job.local.toml  # untracked — local overrides (secrets, machine-specific config)
│   │   └── job.log         # untracked — runtime log
│   ├── cleanup-temp/
│   │   ├── job.toml
│   │   └── job.log
│   └── sync-calendar/
│       ├── job.toml
│       └── job.local.toml
└── handlers/
    ├── send-report.sh
    ├── cleanup-temp.py
    └── sync-calendar.sh
```

## Handlers

Handlers are executable scripts under `~/.config/moadim/handlers/`. The server resolves the `handler` field in `job.toml` to a file in that directory and execs it on each run.

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

Each job folder contains an auto-generated `.gitignore` that excludes `*.local.*` and `*.log` files — no manual ignore setup needed.

### `job.toml`

Tracked configuration — schedule, handler, and shared metadata.

```toml
# ~/.config/moadim/jobs/daily-report/job.toml

schedule = "0 30 9 * * 1-5 *"   # cron expression (seconds field required)
handler  = "send-report"         # filename in ~/.config/moadim/handlers/ (no extension)
enabled  = true                  # omit to default to true

[metadata]
recipient = "team@example.com"
timezone  = "Asia/Jerusalem"
```

| Field        | Type   | Required | Description |
|--------------|--------|----------|-------------|
| `schedule`   | string | yes      | Cron expression. Supports `@daily`, `@hourly`, etc. |
| `handler`    | string | yes      | Script name in `handlers/` (without extension) |
| `enabled`    | bool   | no       | Defaults to `true`. Set `false` to pause without deleting |
| `[metadata]` | table  | no       | Key/value pairs passed to the handler as `MOADIM_*` env vars |

### `job.local.toml`

Untracked overrides — machine-specific values, secrets, or anything that should not be committed. Loaded after `job.toml`; local values win on conflict.

```toml
# ~/.config/moadim/jobs/daily-report/job.local.toml

enabled = false           # overrides job.toml enabled = true → job is disabled

[metadata]
api_key = "sk-..."        # secret — never commit
recipient = "me@local"    # overrides job.toml recipient
```

Any field valid in `job.toml` can be overridden. If both files set `enabled`, the local file wins.

### `job.log`

Append-only log written by the server on each run. Never committed.

```
2026-06-11T09:30:00Z [daily-report] run started
2026-06-11T09:30:01Z [daily-report] run finished OK (1.2s)
```

## Running

### Native server

```sh
cargo run
```

Starts on `http://127.0.0.1:5784`. REST and MCP share the same port.  
The server reads `~/.config/moadim/jobs/` on startup and watches for changes.

## GraphQL

Endpoint: `http://localhost:5784/graphql`

- `GET /graphql` — interactive GraphiQL playground (open in browser)
- `POST /graphql` — execute queries and mutations

Full schema SDL is auto-generated at build time — see [`apis/graphql.graphql`](apis/graphql.graphql).

### Example queries

```graphql
# list all jobs with handler registration status
query {
  cronJobs {
    id schedule handler enabled handlerRegistered
  }
}

# create a job
mutation {
  createCronJob(input: {
    schedule: "0 30 9 * * 1-5 *"
    handler:  "send-report"
    metadata: { recipient: "team@example.com" }
  }) {
    id schedule
  }
}

# disable a job
mutation {
  updateCronJob(id: "...", input: { enabled: false }) {
    id enabled
  }
}

# delete a job
mutation {
  deleteCronJob(id: "...")
}
```

## MCP usage

The server exposes an MCP endpoint at `http://localhost:5784/mcp`. Connect any MCP-compatible client.

### Claude Code

```sh
claude mcp add --transport http moadim http://localhost:5784/mcp
```

### Any MCP client

```
transport: streamable-http
url:       http://localhost:5784/mcp
```

## API

Full interface definitions are auto-generated at build time — see the [`apis/`](apis/) folder.
