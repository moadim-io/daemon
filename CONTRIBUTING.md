# Contributing

By participating in this project you agree to abide by our
[Code of Conduct](CODE_OF_CONDUCT.md).

## Prerequisites

| Tool | Purpose |
| --- | --- |
| [Rust stable](https://rustup.rs/) | Build the daemon |
| [Trunk](https://trunkrs.dev/) | Build the Yew UI (`cargo install --locked --version 0.21.14 trunk` — pinned to match CI, see [`prebuilt-ui.yml`](.github/workflows/prebuilt-ui.yml)) |
| `wasm32-unknown-unknown` target | UI target (`rustup target add wasm32-unknown-unknown`) |
| [`typos`](https://github.com/crate-ci/typos) | Spell check, run by the pre-commit hook (`make spell` installs it automatically) |
| [`cargo-llvm-cov`](https://github.com/taiki-e/cargo-llvm-cov) + `llvm-tools-preview` | 100% line-coverage gate, enforced by the pre-push hook (`cargo install cargo-llvm-cov && rustup component add llvm-tools-preview`) |
| [`linecheck`](https://crates.io/crates/linecheck) | 500-line-per-file gate over `src/` and `ui/src/`, enforced by the pre-push hook and CI's `linecheck` job (`cargo install linecheck`) |
| [`actionlint`](https://github.com/rhysd/actionlint) (with `shellcheck` on `PATH`) | Validates `.github/workflows/*.yml` and the shell in their `run:` blocks; enforced in CI by [`actionlint.yml`](.github/workflows/actionlint.yml) |
| [pnpm](https://pnpm.io/installation) | Runs [Changesets](https://github.com/changesets/changesets) (`pnpm install` once, then `pnpm changeset`) — see [Workflow](#workflow) below |

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
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo llvm-cov --fail-under-lines 100 --ignore-filename-regex 'src/main\.rs'
linecheck --max-lines 500 $(find src ui/src -name '*.rs')
```

Use `--workspace` for both clippy and test, matching the pre-push hook —
bare `cargo clippy`/`cargo test` skip the `ui` member crate entirely (this is
a non-virtual workspace), so they can pass locally yet miss a real `ui`
regression. `cargo llvm-cov` runs the test suite with instrumentation and
enforces 100% line coverage (excluding `main.rs`), but is deliberately scoped
to the root package only — the `ui` crate is a Yew/WASM UI that isn't held to
that floor, so `cargo test --workspace` above is what actually exercises its
own test suite. `linecheck` keeps any single `.rs` file under `src/` or
`ui/src/` from growing past 500 lines — a convention two independently green
PRs can each respect yet still blow past together, since it isn't a required
branch-protection check (see the `linecheck` job in
[`lint.yml`](.github/workflows/lint.yml)).

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

Lint the workflow files under `.github/workflows/` (YAML syntax, `${{ }}`
expressions, the `needs`/`if`/matrix job graph, action input names, and,
via `shellcheck`, every embedded `run:` block) with
[`actionlint`](https://github.com/rhysd/actionlint):

```sh
brew install actionlint shellcheck   # or: bash <(curl -s https://raw.githubusercontent.com/rhysd/actionlint/main/scripts/download-actionlint.bash)
actionlint
```

`actionlint` picks up `shellcheck` from `PATH` automatically if it's
installed; without it, shell-script findings in `run:` blocks are silently
skipped. This mirrors the CI gate in
[`actionlint.yml`](.github/workflows/actionlint.yml), so a clean local run
means the CI job will pass too.

## Reporting security issues

Found a vulnerability? **Do not open a public issue.** See
[`SECURITY.md`](SECURITY.md) for the private disclosure process.

## Architecture at a glance

The daemon (`src/`) is an [Axum](https://github.com/tokio-rs/axum) server that
exposes the same routine (agent-scheduling) functionality over three interfaces on one port:

- **REST** — handlers in `src/routes/http.rs`
- **MCP** — handlers in `src/routes/mcp.rs`
- **UI** — a separate Yew/WASM crate in `ui/`, embedded at build time

Routines are persisted to the OS crontab so they run on schedule. See
[`Architecture.md`](Architecture.md) for the full picture.

## Tests

```sh
cargo test --workspace
```

`--workspace` matters here too: this is a non-virtual workspace, so a bare
`cargo test` silently skips the `ui` member crate's tests.

Tests must live in `*_tests.rs` sibling files, **not** inline
`#[cfg(test)] mod foo { … }` blocks — the pre-push hook rejects inline blocks.
A colocated module reference is fine:

```rust
#[cfg(test)]
#[path = "service_tests.rs"]
mod service_tests; // points at service_tests.rs
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
3. Note user-facing changes with a changeset: run `pnpm changeset`, pick a bump
   type (patch/minor/major), and write a summary in Keep a Changelog style
   (e.g. start it with `### Added`/`### Changed`/`### Fixed` if it doesn't
   obviously fall under the last one used) — that summary is what ends up in
   `CHANGELOG.md` verbatim. Commit the generated `.changeset/*.md` file
   alongside your change. The pre-push hook (and the CI `unreleased-entry`
   check) reject a push that touches `src/` or `ui/` without an accompanying
   changeset file. For a deliberately undocumented change — e.g. a pure
   internal refactor with no user-facing effect — bypass the local hook with
   `SKIP_CHANGELOG=1 git push`; the in-repo equivalent on the PR is the
   `skip-changelog` label.
4. Open a PR against `main`; fill in what changed and why.

## Releasing

See [RELEASING.md](RELEASING.md) for the full walkthrough, the current
manual step, and the reasoning behind how this pipeline is built. Short
version below.

Releases are driven by [Changesets](https://github.com/changesets/changesets).
Changeset files accumulate silently on `main` as PRs land (each one required
by the `unreleased-entry` check above) until someone decides it's time to
ship: trigger [`cut-release.yml`](.github/workflows/cut-release.yml) —
`gh workflow run cut-release.yml`, or "Run workflow" on the Actions tab. It
bumps `package.json`, syncs that version into `Cargo.toml`/`Cargo.lock`
([`scripts/release/version-and-sync.mjs`](scripts/release/version-and-sync.mjs)),
rolls the pending changesets into a new dated `CHANGELOG.md` section, verifies
the result through the same lint/test gates a PR would get, and pushes it
straight to `main` — no PR. (There used to be a bot-maintained "Version
Packages" PR instead; it required GitHub Actions to be allowed to open PRs,
which this org disables, so it never actually worked. See #849.)

To cut one manually instead (e.g. a hotfix, or the workflow is unavailable):

1. `pnpm version-packages` — runs the same bump + sync locally.
2. Review the diff (`package.json`, `Cargo.toml`, `Cargo.lock`, `CHANGELOG.md`).
3. Commit, open a PR, and merge to `main`.

Either way, on landing on `main`, [`auto-release.yml`](.github/workflows/auto-release.yml)
detects the new version, pushes the `vx.y.z` tag, then publishes to crates.io
([`publish.yml`](.github/workflows/publish.yml)) and cuts the GitHub Release
([`release.yml`](.github/workflows/release.yml)). No manual tag push. The tag
must not already exist, and `Cargo.toml`'s version must match the topmost
changelog heading. Pushing a `v*` tag by hand still works as a fallback.

`publish.yml` authenticates to crates.io via [Trusted Publishing](https://crates.io/docs/trusted-publishing)
(OIDC) — no `CARGO_REGISTRY_TOKEN` secret involved.

## Code conventions

- New REST routes go in `src/routes/http.rs`; register them in the router
  builder there (the `.route(...)` chain). New MCP tools go in
  `src/routes/mcp.rs`.
- Error variants belong in `src/error.rs` (`AppError`); fallible handlers
  return `Result<_, AppError>`, which converts to the right HTTP status.
- No `unwrap()` in handler paths — propagate errors via `AppError`.
- `apis/openapi.json` is generated at build time — never edit it by hand.
- `prebuilt.html` is a generated, committed artifact: `build.rs` inlines the
  compiled Yew UI (`ui/`) into it via `trunk build --release`, and it's the
  fallback used whenever `trunk` isn't installed (notably the `cargo install
  moadim` path). Regenerate it after any `ui/` change with the pinned trunk
  version above (`cargo build` rebuilds and overwrites it) and commit the
  result. [`prebuilt-ui.yml`](.github/workflows/prebuilt-ui.yml) fails a PR
  that changes `ui/**` without a matching `prebuilt.html` update — including
  one regenerated with an unpinned, newer trunk than CI's.

## Commit messages

Conventional Commits: `type(scope): subject`.

```text
feat(routines): add pause/resume endpoint
fix(sync): handle missing crontab gracefully
docs: correct contributor setup steps
```

Types: `feat`, `fix`, `chore`, `refactor`, `test`, `docs`.
