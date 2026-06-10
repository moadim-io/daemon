# Contributing

## Prerequisites

| Tool | Purpose |
| --- | --- |
| [Rust stable](https://rustup.rs/) | Native build |
| `wasm32-unknown-unknown` target | WASM build (`rustup target add wasm32-unknown-unknown`) |
| [wasm-pack](https://rustwasm.github.io/wasm-pack/) | Package WASM for the browser |

## Setup

```sh
git clone https://github.com/moadim-io/server
cd server
cargo build
rustup target add wasm32-unknown-unknown
```

Run tests before any commit:

```sh
cargo test
cargo clippy -- -D warnings
cargo fmt --check
```

## Tests

```sh
cargo test
```

## Workflow

1. Branch from `main` — name it `feat/...`, `fix/...`, or `chore/...`.
2. Keep commits focused; one logical change per commit.
3. Open a PR against `main`; fill in what changed and why.

## Code conventions

- `#[cfg(not(target_arch = "wasm32"))]` gates all native-only code.
- `#[cfg(target_arch = "wasm32")]` gates all WASM-only code.
- New routes go in `handlers.rs`; register them in `server.rs`.
- `apis/` is auto-generated at build time — never edit files there directly.
- New WASM exports go in `wasm.rs`; prefix with `wasm_`.
- Error variants belong in `error.rs`; use `AppResult<T>` in handlers.
- No `unwrap()` in handler paths — propagate errors via `AppResult`.

## Adding a cron-job field

1. Add to `CronJob` struct in `cron_jobs.rs`.
2. Add an `Option<T>` field to `UpdateRequest`.
3. Apply the update in the `update` handler and reflect the change in crontab.
4. Add a unit test in the `#[cfg(test)]` block.

Cron entries are persisted in the OS crontab — use `crontab -e` / `crontab -l` to inspect state during development. The server must be able to invoke `crontab` on the host.

## WASM build

```sh
wasm-pack build --target web
```

Output lands in `pkg/`. Load `pkg/server.js` in a browser module script.

## WASM exports

| Function | Signature | Description |
| --- | --- | --- |
| `wasm_init()` | `() → void` | Initialize logging; call once on load |
| `wasm_query_health()` | `() → Promise<string>` | `GET /health` → JSON string |
| `wasm_echo(msg)` | `(string) → Promise<string>` | `POST /echo` → JSON string |
| `wasm_get_info()` | `() → Promise<string>` | `GET /info` → JSON string |
| `wasm_mode()` | `() → string` | Returns `"wasm"` |
| `wasm_checksum(s)` | `(string) → string` | djb2 hash as 8-char hex |
| `wasm_reverse(s)` | `(string) → string` | Unicode-aware reversal |
| `wasm_uppercase(s)` | `(string) → string` | Uppercase via Rust stdlib |

## WASM conventions

Keep WASM exports pure or async-fetch only. No native-only deps (`actix-web`, `tokio`, `uuid`) may appear in `wasm.rs`.

## Commit messages

Conventional Commits: `type(scope): subject`.

```text
feat(cron): add pause/resume endpoint
fix(wasm): handle missing window gracefully
chore: bump actix-web to 4.5
```

Types: `feat`, `fix`, `chore`, `refactor`, `test`, `docs`.
