# Contributing

## Prerequisites

| Tool | Purpose |
| --- | --- |
| [Rust stable](https://rustup.rs/) | Build and test |

## Setup

```sh
git clone https://github.com/moadim-io/server
cd server
cargo build
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

- New routes go in `routes/http.rs`; register them in `build_app`.
- `apis/` is auto-generated at build time — never edit files there directly.
- Error variants belong in `error.rs`; use `AppResult<T>` in handlers.
- No `unwrap()` in handler paths — propagate errors via `AppResult`.

## Adding a cron-job field

1. Add to `CronJob` struct in `cron_jobs.rs`.
2. Add an `Option<T>` field to `UpdateRequest`.
3. Apply the update in the `update` handler and reflect the change in crontab.
4. Add a unit test in the `#[cfg(test)]` block.

Cron entries are persisted in the OS crontab — use `crontab -e` / `crontab -l` to inspect state during development. The server must be able to invoke `crontab` on the host.

## Commit messages

Conventional Commits: `type(scope): subject`.

```text
feat(cron): add pause/resume endpoint
fix(storage): handle missing config dir
chore: bump axum to 0.8
```

Types: `feat`, `fix`, `chore`, `refactor`, `test`, `docs`.
