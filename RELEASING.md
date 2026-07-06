# Releasing moadim

How to cut a release today, and the reasoning/scar tissue behind why it
works this way. See also [CONTRIBUTING.md](CONTRIBUTING.md#releasing) for
the short version.

## How to cut a release (current state)

1. Confirm there's something to ship: `.changeset/*.md` files accumulate on
   `main` as PRs land (each required by the `unreleased-entry` CI check).
2. `gh workflow run cut-release.yml` (or Actions tab → "Run workflow").
   This bumps `package.json`/`Cargo.toml`/`Cargo.lock`/`CHANGELOG.md`
   (`pnpm version-packages` →
   [`scripts/release/version-and-sync.mjs`](scripts/release/version-and-sync.mjs)),
   verifies the result via the same `lint.yml`/`test.yml` gates a normal PR
   gets, and pushes a throwaway branch `ci/version-bump-<run id>` — it does
   **not** touch `main` itself.
3. **Manual step, required every time:** open a PR from that branch onto
   `main` and merge it. GitHub even prints a ready-made URL in the workflow
   log / `git push` output. E.g.:
   ```
   gh pr create --base main --head ci/version-bump-<run id> \
     --title "chore(release): version packages"
   gh pr merge --squash --delete-branch <pr number>
   ```
   No approval is required (`required_approving_review_count: 0` in the
   ruleset below) — once the 5 required checks are green, it merges
   immediately.
4. On merge, [`auto-release.yml`](.github/workflows/auto-release.yml)
   detects the `Cargo.toml` bump, pushes the `vX.Y.Z` tag, and hands off to
   [`publish.yml`](.github/workflows/publish.yml) (crates.io) and
   [`release.yml`](.github/workflows/release.yml) (GitHub Release). Fully
   automatic from here.

Step 3 is the one piece that isn't push-button yet — see "What's still
manual" below for why, and the options for closing it.

## Why it works this way (things that bit us cutting v0.19.1)

**`changeset-release.yml` had never once succeeded, since the day changesets
was introduced.** It used `changesets/action` to keep a standing "Version
Packages" PR up to date, authenticated with a `RELEASE_PAT` secret that was
never actually provisioned (only `CARGO_REGISTRY_TOKEN` existed). Every run
failed with `Input required and not supplied: token`. Every prior release
(e.g. #847) was actually a human running `pnpm version-packages` locally and
opening the PR by hand — the "automation" was decorative. Nobody noticed
because the fallback (a human doing it manually) always worked.

**GitHub Actions cannot create pull requests on this org — confirmed via
API, not assumption.** `gh api repos/moadim-io/daemon/actions/permissions/workflow`
shows `can_approve_pull_request_reviews: false`. Attempting to set it
returns `409 Conflict: "The organization does not allow GitHub Actions to
create or approve pull requests"` — it's an **org-level** policy
(`moadim-io`), not a repo-level one, and fixing it needs `admin:org` scope.
Two dead ends hit trying to grant that scope in this environment: the `gh`
CLI here authenticates via a `GH_TOKEN` env var, and `gh auth refresh` (which
would normally prompt for the extra scope) refuses to run while `GH_TOKEN` is
set — it has to be unset in the *human's* shell first, not something doable
from an agent session.

**`changesets/action` has no direct-to-main mode.** Confirmed against its
docs/`action.yml`: `commitMode` (`git-cli`/`github-api`) only changes *how*
it pushes to its own PR branch, not whether a PR exists at all. It is
fundamentally PR-only. So there's no config flag that sidesteps the org
policy above — the tool has to be replaced, not reconfigured, to avoid
needing a PR-creation-capable credential.

**Skipping the PR entirely doesn't work either — `main` has a repository
*ruleset*, a separate mechanism from classic branch protection.**
`gh api repos/.../branches/main/protection` returns `404 Branch not
protected` — that API only sees the old-style protection, and it's easy to
conclude from that alone that `main` is wide open. It isn't:
`gh api repos/.../rules/branches/main` shows a `pull_request` rule (required
checks: `clippy`, `rustfmt`, `typos`, `unreleased-entry`, `cargo test`; 0
required approvals) plus `non_fast_forward` and `deletion` rules. A direct
`git push <sha>:main` — which is what the first version of `cut-release.yml`
tried — is rejected server-side with `GH013: Changes must be made through a
pull request`, for *any* pusher, Actions or human. **Check both APIs before
concluding a branch is unprotected.**

**What *is* true: a plain tag push is not covered by that ruleset.**
`auto-release.yml`'s tag job pushes `vX.Y.Z` with the default
`GITHUB_TOKEN` and no PR, and it just works — rulesets target branch refs by
name pattern, not `refs/tags/*`. This means `publish.yml`/`release.yml`
could in principle be triggered straight off a tag pushed from the verified
throwaway branch, without ever landing on `main` — but doing that as a
*replacement* for step 3 would leave `main`'s `Cargo.toml`/`CHANGELOG.md`
and the consumed `.changeset/*.md` files permanently out of sync with what's
actually released, corrupting the next cycle's bump computation. If this
gets automated further, tag-directly-and-land-main-separately (as
non-blocking follow-up housekeeping) is the shape to build, not a full
replacement for landing on main.

**`workflow_call` sidesteps the "Actions-authored events don't trigger other
workflows" loop-guard — but watch `github.workflow`.** GitHub won't let a
PR/push made by the default `GITHUB_TOKEN` cascade into other
`pull_request`/`push`-triggered workflows (anti-recursion, not
configurable). `auto-release.yml` already routes around this for
tag-pushes by calling `publish.yml`/`release.yml` directly via
`uses: ./.github/workflows/....yml`; `cut-release.yml` does the same for
`lint.yml`/`test.yml`. Gotcha hit doing this: inside a `workflow_call`,
`${{ github.workflow }}` resolves to the **calling** workflow's name, not
the called one's. `lint.yml` and `test.yml` both keyed their
`concurrency: group:` off `${{ github.workflow }}-${{ github.ref }}`, so
when both got called from `cut-release.yml` they landed on the identical
group and raced to `cancel-in-progress` each other — the first real
`cut-release.yml` run silently lost its `test` job to this within 1 second
of it starting, and the `land` job correctly no-op'd (a cancelled `needs`
job skips dependents) rather than merging half-verified. Fixed by keying
groups off a literal string per file instead.

**The version-bump script had a silent, permanent gap: `docs/moadim.1` was
never synced.** A test (`man_page_version_matches_cargo_pkg_version` in
`src/cli_tests.rs`) asserts the hand-maintained man page's `.TH` header
matches `Cargo.toml`'s version — it was always fixed by a human noticing
post-hoc (see #848, a standalone "sync moadim.1" PR). Once the bump was
actually running in CI instead of by hand, this became a hard failure on
the very first real run. `version-and-sync.mjs` now bumps it alongside
`Cargo.toml`. **Lesson: any manual step a human quietly does alongside a
"mechanical" script is a landmine for when that script gets automated —
audit for these before trusting a newly-automated pipeline.**

**`lychee` (the markdown link checker) will always fail on a version-bump
PR, harmlessly.** `CHANGELOG.md`'s compare-link footer references
`.../compare/vOLD...vNEW`, and `vNEW` doesn't exist as a tag until *after*
this PR merges and `auto-release.yml` runs. It's not in the ruleset's
required-checks list, so it doesn't block anything — just expected noise.

**Housekeeping gotchas re-hit throughout:** `prebuilt.html` regenerates on
any `cargo build`/`cargo check` and must never be `git add -A`'d into a
commit (always `git status`/`git checkout -- prebuilt.html` before
staging); pushes touching `.github/workflows/*` fail over the HTTPS
`origin` remote (`refusing to allow an OAuth App to ... without workflow
scope`) and need `git push git@github.com:moadim-io/daemon.git ...` instead.

## What's still manual, and the options for closing it

Step 3 (`gh pr create` + merge) needs a human (or a credential distinct
from the default `GITHUB_TOKEN`) every release cycle, because:
- Actions can't create PRs here (org policy, needs `admin:org` to fix — see
  above), and
- `main`'s ruleset requires a PR to exist regardless of who's pushing.

Options, none implemented yet:
1. **Org admin flips the policy** (`github.com/organizations/moadim-io/settings/actions`
   → "Allow GitHub Actions to create and approve pull requests"). Zero
   ongoing credential, but needs someone with actual org-admin rights to
   click it — blocked in-session because the available `gh` token couldn't
   even *read* the org's current setting (403, needs `admin:org`).
2. **Fine-grained `RELEASE_PAT`**, scoped to just this repo (Contents +
   Pull requests, read/write). Simplest to set up
   (`gh secret set RELEASE_PAT --repo moadim-io/daemon`), but it's a secret
   someone has to own, scope correctly, and rotate before it expires.
3. **GitHub App installation token.** More setup than a PAT (create the
   App, install it, store its private key as a secret), but doesn't expire
   the same way and isn't tied to a person's account. GitHub's own
   recommended pattern for this exact "PR that needs to trigger other
   workflows" case.
4. **Keep the manual step, decouple it from the release itself.** Tag and
   publish directly off the verified throwaway branch (tags aren't covered
   by the ruleset, confirmed above) so the release ships the moment
   `cut-release.yml` finishes; land the same commit on `main` as a
   separate, non-urgent PR whenever convenient. Turns the manual step from
   "blocks the release" into "keeps `main`'s changelog tidy," which can lag
   without hurting anything downstream.
