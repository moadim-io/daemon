# moadim-server

Rust server that exposes the same functionality over two protocols simultaneously:

- **REST** (`http://localhost:8080`) — standard HTTP API for browsers, CLI tools, and services
- **MCP** (`http://localhost:8081/mcp`) — [Model Context Protocol](https://modelcontextprotocol.io) for AI agents (Claude, etc.)

## Features

- Same cron-job, health, and echo logic reachable via REST and MCP
- Cron-job CRUD backed by the OS cron job system
- API interfaces auto-generated at build time into `apis/`
- Static browser client served from `static/index.html`

## Running

### Native server

```sh
cargo run
```

Starts on `http://127.0.0.1:8080`. The browser client is served from `/`.

## API

Full interface definitions are auto-generated at build time — see the [`apis/`](apis/) folder.
