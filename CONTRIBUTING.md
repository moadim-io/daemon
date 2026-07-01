# Contributing

By participating in this project you agree to abide by our
[Code of Conduct](CODE_OF_CONDUCT.md).

## Prerequisites

| Tool | Purpose |
| --- | --- |
| [Rust stable](https://rustup.rs/) | Build the daemon |
| [Trunk](https://trunkrs.dev/) | Build the Yew UI (`cargo install trunk`) |
| `wasm32-unknown-unknown` target | UI target (`rustup target add wasm32-unknown-unknown`) |
| [`typos`](https://github.com/crate-ci/typos) | Spell check, run by the pre-commit hook (`make spell` installs it automatically) |
| [`cargo-llvm-cov`](https://github.com/taiki-e/cargo-llvm-cov) + `llvm-tools-preview` | 100% line-coverage gate, enforced by the pre-push hook (`cargo install cargo-llvm-cov && rustup component add llvm-tools-preview`) |

The `wasm32` target and Trunk are only needed when working on the browser UI
(`ui/`). The daemon itself is a native binary and builds without them.

## Setup

```sh
git clone https://github.com/moadim-io/daemon
cd daemon
cargo build
```

Run the checks the pre-push hook enforces before any push:

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo llvm-cov --fail-under-lines 100 --ignore-filename-regex 'src/main\.rs'
```

Use `--all-targets -- -D warnings` for clippy, exactly as the pre-push hook and
the CI lint gate do — bare `cargo clippy` skips test/example/bench code and only
warns, so it can pass locally yet fail the hook and CI. The `cargo llvm-cov`
command runs the test suite with instrumentation and enforces 100% line coverage
(excluding `main.rs`); it subsumes a bare `cargo test`, so running it is enough
to satisfy both the test and coverage gates in one pass.

Enable the bundled git hooks once per clone:

```sh
git config core.hooksPath .githooks
```

The **pre-commit** hook spell-checks the tree with
[`typos`](https://github.com/crate-ci/typos); the **pre-push** hook runs the
format/lint/coverage gates below. Spell-check the tree on demand with:

```sh
make spell
```

`make spell` installs `typos-cli` if it's missing, then runs `typos` against
the repo root — you don't need to know the crate/binary name to run it.

Generated and vendored files (`prebuilt.html`, lockfiles, `apis/openapi.json`,
`schemas/`) are excluded in `typos.toml`. To accept a real word that `typos`
flags, add it to `[default.extend-words]` there.

## Reporting security issues

Found a vulnerability? **Do not open a public issue.** See
[`SECURITY.md`](SECURITY.md) for the private disclosure process.

## Architecture at a glance

The daemon (`src/`) is an [Axum](https://github.com/tokio-rs/axum) server that
exposes the same cron-job functionality over three interfaces on one port:

- **REST** — handlers in `src/routes/http.rs`
- **MCP** — handlers in `src/routes/mcp.rs`
- **UI** — a separate Yew/WASM crate in `ui/`, embedded at build time

Jobs are persisted to the OS crontab so they run on schedule. See
[`Architecture.md`](Architecture.md) for the full picture.

## Tests

```sh
cargo test
```

Tests must live in `*_tests.rs` sibling files, **not** inline
`#[cfg(test)] mod foo { … }` blocks — the pre-push hook rejects inline blocks.
A colocated module reference is fine:

```rust
#[cfg(test)]
mod cron_jobs_tests; // semicolon, points at cron_jobs_tests.rs
```

The pre-push hook also requires 100% line coverage (excluding `main.rs`) via
[`cargo-llvm-cov`](https://github.com/taiki-e/cargo-llvm-cov):

```sh
cargo install cargo-llvm-cov
rustup component add llvm-tools-preview
cargo llvm-cov --fail-under-lines 100 --ignore-filename-regex 'src/main\.rs'
```

## Workflow

1. Branch from `main` — name it `feat/...`, `fix/...`, `chore/...`, or `docs/...`.
2. Keep commits focused; one logical change per commit.
3. Note user-facing changes under `## [Unreleased]` in
   [`CHANGELOG.md`](CHANGELOG.md) (Keep a Changelog format). The pre-push hook
   (and the CI `unreleased-entry` check) reject a push that touches `src/` or
   `ui/` without a matching `CHANGELOG.md` edit. For a deliberately undocumented
   change — e.g. a pure internal refactor with no user-facing effect — bypass
   the local hook with `SKIP_CHANGELOG=1 git push`; the in-repo equivalent on
   the PR is the `skip-changelog` label.
4. Open a PR against `main`; fill in what changed and why.

## Releasing

Releases are automated. To cut one, open a PR that bumps the package version:

1. Bump `version` in `Cargo.toml` (and `Cargo.lock`).
2. Promote the `## [Unreleased]` entries in [`CHANGELOG.md`](CHANGELOG.md) to a
   `## [x.y.z] - YYYY-MM-DD` section and add the compare link.
3. Merge to `main`.

On merge, [`auto-release.yml`](.github/workflows/auto-release.yml) detects the
new version, pushes the `vx.y.z` tag, then publishes to crates.io
([`publish.yml`](.github/workflows/publish.yml)) and cuts the GitHub Release
([`release.yml`](.github/workflows/release.yml)). No manual tag push. The tag
must not already exist, and `Cargo.toml`'s version must match the topmost
changelog heading. Pushing a `v*` tag by hand still works as a fallback.

## Code conventions

- New REST routes go in `src/routes/http.rs`; register them in the router
  builder there (the `.route(...)` chain). New MCP tools go in
  `src/routes/mcp.rs`.
- Error variants belong in `src/error.rs` (`AppError`); fallible handlers
  return `Result<_, AppError>`, which converts to the right HTTP status.
- No `unwrap()` in handler paths — propagate errors via `AppError`.
- `apis/openapi.json` and `schemas/job.schema.json` are generated at build time
  — never edit them by hand.

## Adding a cron-job field

1. Add the field to the `CronJob` struct in `src/cron_jobs.rs`.
2. Add a matching `Option<T>` field to `UpdateRequest`.
3. Apply the update in the `update` handler and reflect the change in the
   crontab sync.
4. Add a unit test in the `cron_jobs_tests.rs` sibling file.

Cron entries are persisted in the OS crontab — use `crontab -e` / `crontab -l`
to inspect state during development. The daemon must be able to invoke
`crontab` on the host.

## Commit messages

Conventional Commits: `type(scope): subject`.

```text
feat(cron): add pause/resume endpoint
fix(sync): handle missing crontab gracefully
docs: correct contributor setup steps
```

Types: `feat`, `fix`, `chore`, `refactor`, `test`, `docs`.
