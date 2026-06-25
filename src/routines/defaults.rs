//! Built-in default routines, seeded and kept current on startup.
//!
//! Mirrors [`super::ensure_default_agents`]: on startup the daemon ensures every built-in routine
//! exists, then inserts it into the in-memory store so the crontab sync schedules it.
//!
//! The daemon **owns** the content of its defaults — schedule, agent, and prompt are refreshed from
//! the built-in spec on every start, so improvements ship on upgrade. The one field the daemon never
//! overrides is [`Routine::enabled`]: a new default is created enabled, but if the user has toggled
//! an existing default off it stays off across restarts.
//!
//! A default that is absent from the store (never seeded, or deleted while the daemon was stopped)
//! is (re)created enabled. Suppressing re-add after an explicit delete (e.g. a "removed defaults"
//! marker) is tracked as a follow-up.

use crate::utils::lock::LockRecover;
use uuid::Uuid;

use crate::cron_jobs::normalize_schedule;
use crate::routine_storage::write_routine;
use crate::utils::time::now_secs;

use super::command::slugify;
use super::model::{Routine, RoutineStore};

/// A built-in routine specification: the daemon-owned content reconciled onto disk each startup.
struct DefaultRoutine {
    /// Human name; slugified to name the routine directory, workbench, and tmux session.
    title: &'static str,
    /// Cron expression (local system timezone). Normalized through [`normalize_schedule`].
    schedule: &'static str,
    /// Agent registry key to launch (must match a config under `~/.config/moadim/agents/`).
    agent: &'static str,
    /// Task prompt handed to the agent.
    prompt: &'static str,
}

/// Prompt for the daily `moadim` cargo update routine.
const UPDATE_MOADIM_PROMPT: &str = "\
Ensure the locally installed `moadim` cargo package is up to date, and update it if it is not.

Steps:
1. Find the installed version: `cargo install --list | grep '^moadim '` (no output means it is not installed).
2. Find the latest published version on crates.io: `cargo search moadim --limit 1`.
3. If `moadim` is not installed, or the installed version is older than the latest published version, run `cargo install moadim --force` to update it.
4. If it is already on the latest version, make no changes.

Report which versions you found and whether an update was performed.
";

/// Prompt for the daily self-improvement routine.
const THE_1_PERCENT_PROMPT: &str = "\
You are \"The 1 Percent\" — a self-improving routines agent. Each day you make the user's \
automation portfolio exactly 1% better: one focused, high-quality improvement, nothing more.

## Step 1: Environment check

Run:
```bash
git -C ~/.config/moadim rev-parse --is-inside-work-tree 2>/dev/null && echo IS_REPO || echo NOT_REPO
```

**If NOT_REPO:** use the moadim MCP tool `list_routines` to find the routine whose title is \
\"The 1 Percent\", then call `update_routine` with `enabled: false` on that ID. \
Log: \"Routines folder is not a git repository — self-disabled.\" Then stop.

**If IS_REPO:** continue.

## Step 2: Analyze the user's automation portfolio

Use the moadim MCP tool `list_routines` to fetch all routines. For each one note: title, \
schedule, agent, enabled, machines, last_scheduled_trigger_at, last_manual_trigger_at, and the \
full prompt text.

Evaluate across these dimensions:
- **Coverage gaps** — life/work areas with zero automation (health, finances, learning, \
communication, code quality, etc.)
- **Redundancy** — two or more routines with overlapping intent that could be merged
- **Dead weight** — disabled or never-triggered routines with no clear future use
- **Prompt quality** — vague, ambiguous, or brittle prompts that likely produce poor results
- **Schedule hygiene** — routines firing too often, at bad times, or not often enough
- **Machine targeting** — routines with an empty machines list that will never fire

## Step 3: Choose the single best improvement

Rank by impact-to-effort ratio and pick ONE. Before acting, check for open PRs on the routines \
repo to avoid duplicating a pending proposal:
```bash
origin=$(git -C ~/.config/moadim remote get-url origin 2>/dev/null)
gh pr list --repo \"$origin\" --state open --json title,headRefName 2>/dev/null
```
If an equivalent improvement is already proposed, pick the next-best one.

Good improvement examples:
- Write a complete, precise new routine prompt for an unautomated area
- Rewrite a vague prompt to be specific, testable, and effective
- Merge two redundant routines into one stronger one
- Fix a broken or wasteful schedule expression
- Remove a demonstrably dead routine
- Add a missing machine target so a routine actually fires

## Step 4: Open the PR

```bash
git -C ~/.config/moadim checkout -b 1pct/$(date +%Y%m%d-%H%M)
# Edit/add/delete files under ~/.config/moadim/routines/ as needed.
# Only touch routine.toml files. Do NOT modify moadim daemon config.
git -C ~/.config/moadim add -A
git -C ~/.config/moadim commit -m \"routines: <concise description>\"
git -C ~/.config/moadim push -u origin HEAD
origin=$(git -C ~/.config/moadim remote get-url origin)
gh pr create --repo \"$origin\" \
  --title \"1%: <short description>\" \
  --body \"$(printf '## What changed\\n<one paragraph>\\n\\n## Why this improves the portfolio\\n<one paragraph>\\n\\n## Expected impact\\n<one line>')\"
```

## Report

Output exactly three lines:
1. Env: IS_REPO or NOT_REPO (+ self-disabled if applicable)
2. Improvement chosen: <type> — <one-line description>
3. PR: <url> or \"Skipped: <reason>\"
";

/// Prompt for the weekly token-efficiency routine.
const TOKEN_TRIM_PROMPT: &str = "\
You are \"Token Trim\" — a token-efficiency agent. Each week you analyze the user's \
automation portfolio and open one PR that reduces LLM token consumption without \
degrading output quality.

## Step 1: Environment check

Run:
```bash
git -C ~/.config/moadim rev-parse --is-inside-work-tree 2>/dev/null && echo IS_REPO || echo NOT_REPO
```

**If NOT_REPO:** use the moadim MCP tool `list_routines` to find the routine whose title is \
\"Token Trim\", then call `update_routine` with `enabled: false` on that ID. \
Log: \"Routines folder is not a git repository — self-disabled.\" Then stop.

**If IS_REPO:** continue.

## Step 2: Audit token costs

Use the moadim MCP tool `list_routines` to fetch all routines. For each enabled routine, \
examine the full prompt text and score it on:

- **Redundancy** — repeated instructions, restated context, or examples that duplicate \
  what the LLM already knows
- **Verbosity** — multi-sentence explanations a single precise sentence would cover; \
  enumerations that collapse to a pattern
- **Dead scaffolding** — section headers, bullet lead-ins, or transition phrases that \
  add tokens but no information (\"Please make sure to\", \"As a next step\", \"Note that\")
- **Over-specified steps** — step-by-step breakdowns for tasks the model handles reliably \
  without hand-holding
- **Prompt duplication** — two or more routines whose prompts share large identical or \
  near-identical passages that could be factored out

Estimate a rough token saving for each finding (small < 50 tokens, medium 50–200, large > 200).

## Step 3: Choose the single best improvement

Pick ONE finding with the best saving-to-risk ratio (prefer high saving, low risk of \
degrading output quality). Before acting, check for open PRs:
```bash
origin=$(git -C ~/.config/moadim remote get-url origin 2>/dev/null)
gh pr list --repo \"$origin\" --state open --json title,headRefName 2>/dev/null
```
If an equivalent improvement is already proposed, pick the next-best one.

Good improvement examples:
- Compress a verbose multi-paragraph prompt to a tight, precise equivalent
- Remove a redundant section that duplicates another routine's instructions
- Collapse an over-specified step list into a single goal statement
- Delete dead scaffolding phrases and filler transitions
- Deduplicate a shared preamble that appears across two or more routines

**Constraint:** the rewritten prompt must preserve all observable behavior — same tool \
calls, same outputs, same decision logic. When in doubt, keep the instruction.

## Step 4: Open the PR

```bash
git -C ~/.config/moadim checkout -b token-trim/$(date +%Y%m%d-%H%M)
# Edit the relevant routine.toml / prompt.md file(s) under ~/.config/moadim/routines/.
# Only touch routine prompt files. Do NOT modify moadim daemon config.
git -C ~/.config/moadim add -A
git -C ~/.config/moadim commit -m \"routines: trim token cost in <routine-name>\"
git -C ~/.config/moadim push -u origin HEAD
origin=$(git -C ~/.config/moadim remote get-url origin)
gh pr create --repo \"$origin\" \\
  --title \"token-trim: <short description>\" \\
  --body \"$(printf '## What changed\\n<one paragraph>\\n\\n## Estimated token saving\\n~<N> tokens per run\\n\\n## Risk\\n<none|low|medium> — <one line>')\"
```

## Report

Output exactly three lines:
1. Env: IS_REPO or NOT_REPO (+ self-disabled if applicable)
2. Improvement chosen: <type> — <one-line description> (~<N> tokens saved per run)
3. PR: <url> or \"Skipped: <reason>\"
";

/// Built-in default routines, reconciled onto disk on every startup.
const DEFAULT_ROUTINES: &[DefaultRoutine] = &[
    DefaultRoutine {
        title: "Update moadim cargo package",
        // Daily at 09:00 local time.
        schedule: "0 9 * * *",
        agent: "claude",
        prompt: UPDATE_MOADIM_PROMPT,
    },
    DefaultRoutine {
        title: "The 1 Percent",
        // Daily at 08:00 local time — runs before the cargo-update routine.
        schedule: "0 8 * * *",
        agent: "claude",
        prompt: THE_1_PERCENT_PROMPT,
    },
    DefaultRoutine {
        title: "Token Trim",
        // Weekly on Sundays at 07:00 local time — before the daily routines.
        schedule: "0 7 * * 0",
        agent: "claude",
        prompt: TOKEN_TRIM_PROMPT,
    },
];

/// Build a concrete [`Routine`] from a [`DefaultRoutine`] spec, stamping `now` as the create/update
/// time and normalizing the schedule. Kept separate from disk/store mutation so it can be unit
/// tested.
fn materialize(spec: &DefaultRoutine, now: u64) -> Routine {
    Routine {
        id: Uuid::new_v4().to_string(),
        schedule: normalize_schedule(spec.schedule),
        title: spec.title.to_string(),
        agent: spec.agent.to_string(),
        prompt: spec.prompt.to_string(),
        repositories: Vec::new(),
        // Self-assign a fresh default to the machine seeding it, so it actually runs out of the box
        // (an empty `machines` list would leave the default dormant on every machine). On a shared
        // config repo the default is seeded once, on whichever machine starts first; the user can
        // reassign it with `moadim routines update`.
        machines: vec![crate::machine::current_machine()],
        enabled: true,
        source: "managed".to_string(),
        created_at: now,
        updated_at: now,
        last_manual_trigger_at: None,
        last_scheduled_trigger_at: None,
        ttl_secs: None,
        max_runtime_secs: None,
    }
}

/// Reconcile an existing default `cur` against its built-in `spec`, preserving the user's choices.
///
/// Returns `Some(updated)` when a daemon-owned field (schedule, agent, prompt, or the empty
/// repositories list) drifted from the spec and the routine must be rewritten, or `None` when `cur`
/// already matches and no write is needed. The user-owned [`Routine::enabled`] toggle is always
/// carried over from `cur` — so a default the user turned off stays off — as are its `id`,
/// `created_at`, `last_manual_trigger_at`, and `last_scheduled_trigger_at`.
fn reconcile(spec: &DefaultRoutine, cur: &Routine, now: u64) -> Option<Routine> {
    let schedule = normalize_schedule(spec.schedule);
    let up_to_date = cur.schedule == schedule
        && cur.agent == spec.agent
        && cur.prompt == spec.prompt
        && cur.repositories.is_empty();
    if up_to_date {
        return None;
    }
    Some(Routine {
        id: cur.id.clone(),
        schedule,
        title: spec.title.to_string(),
        agent: spec.agent.to_string(),
        prompt: spec.prompt.to_string(),
        repositories: Vec::new(),
        // Machine targeting is user-owned, like `enabled`: carry the existing choice across a
        // spec-driven reconcile so a default reassigned (or unassigned) by the user stays that way.
        machines: cur.machines.clone(),
        enabled: cur.enabled,
        source: "managed".to_string(),
        created_at: cur.created_at,
        updated_at: now,
        last_manual_trigger_at: cur.last_manual_trigger_at,
        last_scheduled_trigger_at: cur.last_scheduled_trigger_at,
        ttl_secs: cur.ttl_secs,
        max_runtime_secs: cur.max_runtime_secs,
    })
}

/// Ensure every built-in default routine exists and matches its spec, then schedule it.
///
/// For each [`DEFAULT_ROUTINES`] entry: if a routine with the same slug is already in `store`, it is
/// refreshed via [`reconcile`] (daemon-owned content updated, the user's `enabled` toggle preserved)
/// and only rewritten when it drifted; otherwise a fresh enabled routine is created. Persists each
/// affected routine (`routine.toml` + `prompt.md` + `.gitignore`) and inserts it into `store` so the
/// subsequent crontab sync schedules it. Best-effort: a write failure is logged and skipped rather
/// than aborting startup. Call once at startup after [`crate::routine_storage::load_store`] and
/// before the crontab sync.
pub fn ensure_default_routines(store: &RoutineStore) {
    for spec in DEFAULT_ROUTINES {
        let slug = slugify(spec.title);
        let existing = store
            .lock_recover()
            .values()
            .find(|routine| slugify(&routine.title) == slug)
            .cloned();
        let routine = match existing {
            Some(cur) => match reconcile(spec, &cur, now_secs()) {
                Some(updated) => updated,
                None => continue,
            },
            None => materialize(spec, now_secs()),
        };
        if let Err(err) = write_routine(&routine) {
            log::warn!(
                "ensure_default_routines: failed to write {:?}: {err}; skipping",
                spec.title
            );
            continue;
        }
        store.lock_recover().insert(routine.id.clone(), routine);
    }
}

#[cfg(test)]
#[path = "defaults_tests.rs"]
mod defaults_tests;
