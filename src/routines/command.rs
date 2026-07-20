//! Prompt composition, slug/shell helpers, and the single-line tmux launch command builder.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use crate::paths::{routine_compiled_prompt_path, routine_scheduled_log_path};
use crate::routine_storage::read_local_env;

use super::agents::AgentCommand;
use super::flags::{list_flags, FlagScope};
use super::model::Routine;

/// Slugify `title` into a filesystem- and tmux-safe path identifier.
///
/// Lowercases, replaces each run of non-alphanumeric characters *inside a path segment* with `-`,
/// preserves `/` as the segment separator, trims empty segments, and returns `"routine"` if empty.
///
/// Unicode-aware: uses [`char::is_alphanumeric`] / [`char::to_lowercase`] rather than the ASCII-only
/// variants, so non-Latin titles (Hebrew, CJK, Cyrillic) and Latin letters with diacritics (`é`,
/// `ü`) keep their content instead of collapsing to the `"routine"` fallback (#262). The path is
/// still shell-quoted wherever it is embedded.
pub(crate) fn slugify(title: &str) -> String {
    let segments: Vec<String> = title
        .split('/')
        .filter_map(|segment| {
            let mut out = String::new();
            let mut prev_dash = false;
            for ch in segment.chars() {
                if ch.is_alphanumeric() {
                    out.extend(ch.to_lowercase());
                    prev_dash = false;
                } else if !prev_dash {
                    out.push('-');
                    prev_dash = true;
                }
            }
            let trimmed = out.trim_matches('-').to_string();
            (!trimmed.is_empty()).then_some(trimmed)
        })
        .collect();
    if segments.is_empty() {
        "routine".to_string()
    } else {
        segments.join("/")
    }
}

/// Literal prefix every routine fire's tmux session name begins with; the remainder is
/// `{slug}-{fire's $TS}` (see the `SESS=` line in [`build_routine_command`]).
pub(crate) const TMUX_SESSION_PREFIX: &str = "moadim-";

/// The tmux session-name prefix shared by every fire of the routine identified by `slug` —
/// `{TMUX_SESSION_PREFIX}{slug}-`, matching every session name [`build_routine_command`] can
/// produce for it regardless of `$TS`. Used by the overlap guard (#514) to detect whether *any*
/// fire of this routine already has a live session, not just one exact `$TS`.
pub(crate) fn tmux_session_prefix(slug: &str) -> String {
    format!("{TMUX_SESSION_PREFIX}{slug}-")
}

/// Compose the `prompt.compiled.local.md` body: a repositories-as-context preamble, an optional
/// `## Goal` section, a routine-origin disclosure block, the prompt, and an "Open flags" section.
/// When the routine lists no repositories the preamble omits the "clone any you need:" sentence
/// and its (otherwise empty) bullet list, so the agent never sees a dangling header promising a
/// repo list with nothing under it.
pub(crate) fn compose_prompt(routine: &Routine) -> String {
    let mut body = String::from("# Workbench\n");
    if routine.repositories.is_empty() {
        body.push_str("You are working in an empty directory.\n");
    } else {
        body.push_str(
            "You are working in an empty directory. These repositories are relevant — clone any you need:\n",
        );
        for repo in &routine.repositories {
            // `write!` into the existing `String` directly rather than `format!` + `push_str`,
            // which would allocate a throwaway `String` per repository just to copy it into
            // `body` immediately after. Writing to a `String` is infallible, so the `Result` is
            // deliberately discarded.
            match &repo.branch {
                Some(branch) => {
                    let _ = writeln!(body, "- {} (branch {})", repo.repository, branch);
                }
                None => {
                    let _ = writeln!(body, "- {}", repo.repository);
                }
            }
        }
    }
    // A short "why" preamble, when set, so the agent has the routine's intent before the task.
    if let Some(goal) = routine
        .goal
        .as_deref()
        .map(str::trim)
        .filter(|text| !text.is_empty())
    {
        body.push_str("\n## Goal\n");
        body.push_str(goal);
        body.push('\n');
    }
    body.push_str("\n## Routine origin disclosure\n\n");
    body.push_str("You act on behalf of the moadim routine named below. In every external, outward-facing communication you produce — GitHub issues, pull requests and comments; Slack messages; emails; any channel a human or third-party system receives — you MUST disclose that the action originates from this moadim routine, naming it.\n\n");
    let _ = writeln!(body, "Routine name: {}", routine.title);
    body.push_str("\n---\n");
    body.push_str(&routine.prompt);
    body.push('\n');

    let flags = list_flags(&slugify(&routine.title));
    if !flags.is_empty() {
        body.push_str("\n---\n# Open flags\n\nRaised on a previous run and not yet resolved:\n\n");
        for flag in &flags {
            let scope = match flag.scope {
                FlagScope::General => "general",
                FlagScope::Local => "local",
            };
            let _ = writeln!(
                body,
                "- **{}** ({scope}): {}",
                flag.flag_type, flag.description
            );
        }
    }
    body
}

/// Substitute `{workbench}`, `{prompt_file}`, and `{prompt}` placeholders in `s`.
///
/// `{prompt}` expands to a shell command substitution that reads `prompt.md` from the agent's
/// cwd (the workbench), so the full prompt is passed as a single argument to the agent process.
#[allow(
    clippy::literal_string_with_formatting_args,
    reason = "these are literal `String::replace` placeholder tokens, not `format!`-family arguments — there is no formatting macro here to move them into"
)]
pub(crate) fn substitute(template: &str, workbench: &str, prompt_file: &str) -> String {
    template
        .replace("{workbench}", workbench)
        .replace("{prompt_file}", prompt_file)
        .replace("{prompt}", r#""$(cat prompt.md)""#)
}

/// The placeholder tokens [`substitute`] understands.
const KNOWN_PLACEHOLDERS: [&str; 3] = ["{workbench}", "{prompt_file}", "{prompt}"];

/// Return the placeholder-style `{name}` tokens in `arg`.
///
/// A token is a `{`, *not* immediately preceded by `$`, wrapping a lowercase identifier
/// (`[a-z][a-z_]*`), closed by the next `}`. This shape deliberately matches the known
/// placeholders and nothing else: shell constructs like `${HOME}`, `{}`, `{0}`, or `{print $1}`
/// are ignored, so only genuine placeholder typos (`{prompt_fil}`, `{wokbench}`) surface.
fn placeholder_tokens(arg: &str) -> Vec<String> {
    let bytes = arg.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'{' && (i == 0 || bytes[i - 1] != b'$') {
            if let Some(rel) = arg[i + 1..].find('}') {
                let inner = &arg[i + 1..i + 1 + rel];
                if inner.starts_with(|ch: char| ch.is_ascii_lowercase())
                    && inner.chars().all(|ch| ch.is_ascii_lowercase() || ch == '_')
                {
                    out.push(format!("{{{inner}}}"));
                }
                i += 1 + rel + 1;
                continue;
            }
        }
        i += 1;
    }
    out
}

/// Validate that an agent's `args` can actually deliver a prompt and carry no typo'd placeholder.
///
/// Two silent fire-time failures are caught up front (#322):
///
/// * **Typo'd placeholder.** A token like `{prompt_fil}` is left untouched by [`substitute`] and
///   reaches the agent as a literal argument; the task never runs. Any placeholder-style token
///   outside [`KNOWN_PLACEHOLDERS`] is rejected, naming the offender.
/// * **Missing prompt.** If no arg contains `{prompt}` or `{prompt_file}`, the composed prompt is
///   never passed and the agent launches with no task, burning a full run until the watchdog reaps
///   it. At least one prompt placeholder is therefore required.
pub(crate) fn validate_placeholders(args: &[String]) -> Result<(), String> {
    for arg in args {
        for token in placeholder_tokens(arg) {
            if !KNOWN_PLACEHOLDERS.contains(&token.as_str()) {
                return Err(format!(
                    "unknown placeholder {token} in args; supported placeholders are {}",
                    KNOWN_PLACEHOLDERS.join(", ")
                ));
            }
        }
    }
    let delivers_prompt = args
        .iter()
        .any(|arg| arg.contains("{prompt}") || arg.contains("{prompt_file}"));
    if !delivers_prompt {
        return Err(
            "args must include a prompt placeholder ({prompt} or {prompt_file}); \
             otherwise the agent launches with no task"
                .to_string(),
        );
    }
    Ok(())
}

/// Conservative cap on a single inlined `{prompt}` argument, matching Linux's
/// `MAX_ARG_STRLEN` (`32 * PAGE_SIZE` = 128 KiB on the common 4 KiB page size) — the
/// tighter of the two platform limits an inlined prompt is exposed to (macOS's
/// combined arg+env budget, `kern.argmax`, is roughly double). An agent using
/// `{prompt_file}` instead is never subject to this: the prompt reaches the process
/// as a file path, not a single oversized argv entry.
pub(crate) const MAX_INLINE_PROMPT_BYTES: usize = 128 * 1024;

/// Byte length of `routine`'s composed prompt when `agent` would inline it into a
/// single process argument that exceeds [`MAX_INLINE_PROMPT_BYTES`]; `None` when the
/// agent doesn't use `{prompt}` at all, or the composed prompt fits.
///
/// Only agents whose `args` template contains the literal `{prompt}` placeholder are
/// at risk (see [`substitute`]) — `claude`, the shipped default, is one of them
/// (#443). A large composed prompt (routine `prompt` + the repositories preamble +
/// accumulated open flags, see [`compose_prompt`]) then makes the `execve` inside the
/// launch's detached tmux session fail with `E2BIG`, silently no-oping the run
/// instead of erroring anywhere visible.
pub(crate) fn inline_prompt_overflow(routine: &Routine, agent: &AgentCommand) -> Option<usize> {
    if !agent.args.iter().any(|arg| arg.contains("{prompt}")) {
        return None;
    }
    let len = compose_prompt(routine).len();
    (len > MAX_INLINE_PROMPT_BYTES).then_some(len)
}

#[path = "command_path_resolution.rs"]
mod command_path_resolution;
pub(crate) use command_path_resolution::*;

/// Wrap `s` in single quotes for safe inclusion in a POSIX shell command.
pub(crate) fn shell_quote(text: &str) -> String {
    let mut out = String::from("'");
    for ch in text.chars() {
        if ch == '\'' {
            out.push_str("'\\''");
        } else {
            out.push(ch);
        }
    }
    out.push('\'');
    out
}

#[path = "command_system_prompt.rs"]
mod command_system_prompt;
pub(crate) use command_system_prompt::system_prompt_stmts;

/// `true` when `key` is a POSIX-portable shell identifier: `[A-Za-z_][A-Za-z0-9_]*`.
///
/// Shared by `service_validate::validate_env` (the API create/update path, tracked `[env]`) and
/// [`env_export_stmts`] below (defense in depth against a hand-edited, never-API-validated
/// `routine.local.toml`) — see issue #408.
pub(crate) fn is_valid_env_key(key: &str) -> bool {
    let mut chars = key.chars();
    matches!(chars.next(), Some(first) if first.is_ascii_alphabetic() || first == '_')
        && chars.all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
}

/// Build the `export KEY=<shell-quoted value>` statements for `routine`'s resolved environment:
/// the tracked `routine.toml` `[env]` table, overlaid with the untracked `routine.local.toml`
/// sidecar (secrets) — whose keys win on conflict (#408).
///
/// A `BTreeMap` merge keeps the emitted statements in a deterministic, sorted-by-key order (stable
/// test assertions, stable output for anyone reading `launch.log`). Every entry — from either
/// source — is re-checked with [`is_valid_env_key`] and scanned for newlines: `routine.toml` was
/// already validated at create/update time (`service_validate::validate_env`), but
/// `routine.local.toml` is a file a human edits directly on disk and never passes through that
/// check, so a malformed entry there is dropped (with a warning) rather than corrupting the
/// single-line, `;`-joined launch command.
fn env_export_stmts(routine: &Routine) -> Vec<String> {
    let slug = slugify(&routine.title);
    let local_env = read_local_env(&slug);
    let mut merged: BTreeMap<String, String> = BTreeMap::new();
    for (key, value) in routine.env.iter().chain(local_env.iter()) {
        if is_valid_env_key(key) && !value.contains('\n') && !value.contains('\r') {
            merged.insert(key.clone(), value.clone());
        } else {
            log::warn!(
                "routine {:?}: skipping invalid env var {key:?} (from routine.toml or \
                 routine.local.toml — invalid key or a newline in the value)",
                routine.id
            );
        }
    }
    merged
        .into_iter()
        .map(|(key, value)| format!("export {key}={}", shell_quote(&value)))
        .collect()
}

/// Who is launching the routine command — which decides whether the run records a *scheduled*
/// firing.
///
/// Only the OS crontab firing on schedule should append to `scheduled.log`; a manual (on-demand)
/// trigger reuses the very same launch script but must not masquerade as a scheduled fire, or
/// `last_scheduled_trigger_at` would be overwritten every time an operator hits "run now".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TriggerSource {
    /// The OS crontab firing on schedule — records the fire time into `scheduled.log`.
    Scheduled,
    /// An on-demand trigger (UI/API/CLI) — runs the agent but leaves `scheduled.log` untouched, so
    /// the manual run is tracked only via `last_manual_trigger_at` (recorded in-process by
    /// `svc_trigger`, not by this script).
    Manual,
}

/// Build the single-line shell command that creates a workbench and launches the agent in tmux.
///
/// The agent's cwd is the workbench (via `tmux -c`), so `{prompt_file}` resolves to `prompt.md`,
/// `{workbench}` to `.`, and `{prompt}` to the prompt's contents passed as one argument. The prompt
/// reaches the agent as a process argument (not keystrokes), so there is no readiness race. The
/// command is `;`-joined (no newlines) so it fits one crontab line.
///
/// `source` controls whether the script records a scheduled firing: a [`TriggerSource::Scheduled`]
/// run (the crontab) appends to `scheduled.log`, while a [`TriggerSource::Manual`] run omits the
/// append so an on-demand trigger never clobbers `last_scheduled_trigger_at`.
pub(crate) fn build_routine_command(
    routine: &Routine,
    agent: &AgentCommand,
    source: TriggerSource,
) -> String {
    let slug = slugify(&routine.title);
    let prompt_path = routine_compiled_prompt_path(&slug)
        .to_string_lossy()
        .into_owned();
    let scheduled_log_path = routine_scheduled_log_path(&slug)
        .to_string_lossy()
        .into_owned();
    // Resolve through the same seam the reaper (`cleanup/mod.rs`) and the LOGS view
    // (`routines/service.rs`) use, rather than hardcoding `$HOME/.moadim/workbenches`: honoring
    // `MOADIM_HOME_OVERRIDE` here keeps the path a run is launched at in sync with the paths those
    // consumers scan, instead of drifting the moment either side changes.
    let workbenches_base = crate::paths::workbenches_dir()
        .to_string_lossy()
        .into_owned();

    let prompt_file_ref = "prompt.md";
    let workbench_ref = ".";

    let mut invocation = vec![agent.command.clone()];
    for arg in &agent.args {
        invocation.push(substitute(arg, workbench_ref, prompt_file_ref));
    }
    // Routine-level model override, by convention supported as `--model <id>` across the built-in
    // agents (`claude`, `codex`, `hermes`). Appended after the agent's own args so it wins over any
    // default the agent config sets. `shell_quote` guards against the model ID (user input) breaking
    // out of the invocation, which the surrounding `shell_quote(&invocation)` call re-escapes as a
    // whole when it embeds this into the cron line.
    if let Some(model) = &routine.model {
        invocation.push("--model".to_string());
        invocation.push(shell_quote(model));
    }
    let invocation = invocation.join(" ");

    let mut stmts = vec![
        // Owner-only umask for everything this run creates: the workbench dir, the copied
        // `prompt.md`, the appended `CLAUDE.md`, and the tmux-piped `agent.log` (which captures the
        // full agent transcript — cloned repo contents, command output, any printed secrets). Set
        // before the first `mkdir`/`cp`/`tmux` so those artifacts land `0700`/`0600` instead of the
        // login shell's default world-readable umask, matching the daemon's own on-disk posture.
        "umask 077".to_string(),
        // The crontab invokes this script under a *login* shell (`/bin/sh -l`; see
        // `sync::routines::format_routine_line`), so the user's `~/.profile` is sourced first and
        // the agent inherits their environment — GH_TOKEN, API keys and the like — which cron's
        // minimal env (and, on macOS, the GUI-Keychain-less session) otherwise withholds.
        //
        // The curated dirs are *appended* to the profile's PATH (`$PATH:<curated>`), not
        // substituted for it. The profile-sourced `$PATH` therefore keeps precedence, so the
        // version-manager shim dirs a profile prepends (nvm/pyenv/asdf/volta) survive and the agent
        // resolves the node/python the user actually selected. The curated list trails as a
        // fallback, guaranteeing `tmux` and the agent `command` stay resolvable even when the
        // profile's PATH omits their dirs (or the profile sets no PATH at all). `$PATH` is left
        // unquoted so the login shell expands it; only the curated suffix is quoted.
        format!(
            "export PATH=$PATH:{}",
            shell_quote(&cron_path(&agent.command))
        ),
    ];
    // Per-routine env vars (issue #408): the tracked `routine.toml` `[env]` table, overlaid with
    // the untracked `routine.local.toml` sidecar (secrets — its keys win). Emitted right after the
    // curated PATH export and before anything else runs, so they override any profile-inherited
    // value (e.g. a shared `GH_TOKEN`) for this run only, without touching the operator's actual
    // shell environment.
    stmts.extend(env_export_stmts(routine));
    stmts.push(r#"TS="$(date +%s)""#.to_string());
    if source == TriggerSource::Scheduled {
        // Record this scheduled firing. Appends the Unix timestamp as one line to the routine's
        // gitignored `scheduled.log`; the daemon reads the last line back as
        // `last_scheduled_trigger_at` on load. Using `>>` (append) preserves the full run history.
        // Written before the prompt-copy guard below so an aborted run still records that the
        // schedule fired, and best-effort (`|| true`) so a log write failure never blocks launching.
        //
        // A manual ([`TriggerSource::Manual`]) trigger deliberately omits this append: it shares
        // the exact same launch script but is tracked via `last_manual_trigger_at` (recorded
        // in-process by `svc_trigger`), so appending here would conflate an on-demand "run now"
        // with a genuine scheduled fire.
        stmts.push(format!(
            r#"printf '%s\n' "$TS" >> {} || true"#,
            shell_quote(&scheduled_log_path)
        ));
    }
    stmts.extend([
        format!("SLUG={}", shell_quote(&slug)),
        // Collision-resistant run id. `$TS` alone has one-second granularity, so two runs of the
        // *same* routine in the same wall-clock second (a double-clicked "Run now", a `trigger`
        // retry, or a manual trigger landing on the scheduled cron fire) would derive an identical
        // `$WB` and `$SESS`: the second `tmux new-session` fails with "duplicate session" and that
        // run silently no-ops while both clobber the shared workbench files. `$$` is the launching
        // shell's PID — distinct across concurrently-live processes — so each run gets a unique id
        // even within the same second. POSIX-portable (works under `/bin/sh`/dash), filesystem- and
        // shell-safe, and short enough to stay within the single crontab line. `$TS` is kept
        // unchanged above for the scheduled-fire sidecar. The PID is joined with `_` (not `-`) so
        // `parse_workbench_name` can still recover the slug and the trailing-timestamp: slugs are
        // `[a-z0-9-]` only, so `_` is an unambiguous boundary and legacy `{slug}-{secs}` dirs keep
        // parsing. (#411)
        r#"RID="${TS}_$$""#.to_string(),
        format!(r#"WB={}/"$SLUG-$RID""#, shell_quote(&workbenches_base)),
        format!(r#"SESS="{TMUX_SESSION_PREFIX}$SLUG-$RID""#),
        r#"mkdir -p "$WB""#.to_string(),
    ]);

    // Everything from here on runs with stdout/stderr redirected into the workbench itself, so a
    // failure in the setup step or the tmux launch leaves a readable trace instead of being handed
    // to cron's mail spool (silently discarded on the headless hosts this daemon targets — see
    // #375). `$WB` already exists (created by the `mkdir` above), so the redirect target is valid.
    // The `cp`/disclosure guards below still `tee` their own abort reason into `agent.log`
    // explicitly; under this wrapper that message also lands in `launch.log`, which is harmless.
    let mut inner_stmts = Vec::new();
    inner_stmts.extend(system_prompt_stmts(
        &crate::paths::user_prompt_path().to_string_lossy(),
        &routine.title,
        &agent.instructions_file,
    ));
    inner_stmts.extend([
        // Fail-fast if the routine's source prompt is missing. The statements are `;`-joined, so a
        // bare `cp` failure would be ignored and the agent would launch with an empty
        // `"$(cat prompt.md)"` argument — a blank, task-less session. Abort instead, recording the
        // reason in the workbench's agent.log (already created via mkdir) and on stderr.
        format!(
            r#"cp {src} "$WB/prompt.md" || {{ echo "moadim: missing routine prompt {src}; aborting launch" | tee -a "$WB/agent.log" >&2; exit 1; }}"#,
            src = shell_quote(&prompt_path)
        ),
    ]);
    if let Some(setup) = &agent.setup {
        // Fail-fast if the agent's setup step fails, mirroring the `cp prompt.md` guard above. The
        // statements are `;`-joined (no `set -e`), so a bare `setup` failure would be ignored and
        // the agent would launch anyway — typically into the interactive trust/onboarding prompt
        // with no stdin, where it hangs until the watchdog reaps it ~1h later with no diagnostic.
        // Abort instead, recording the reason in agent.log and on stderr. The setup string is
        // inserted verbatim so the agent author controls quoting; `$WB`/`$SESS` are in scope.
        inner_stmts.push(format!(
            r#"{{ {setup}; }} || {{ echo "moadim: agent setup failed; aborting launch" | tee -a "$WB/agent.log" >&2; exit 1; }}"#
        ));
    }
    // Record the agent's exit code once it finishes, so the run-history view (`svc_list_runs`)
    // can tell success from failure instead of only "session ended". `tmux new-session` runs a
    // single quoted string through the pane's default shell, so `;`-appending here shares that
    // same shell and its `$?`. Written to a workbench-*relative* path (`exit_code`, not
    // `$WB/exit_code`): `$WB` is a plain (non-exported) shell variable in the launcher script and
    // is not inherited by the new shell tmux spawns, but the pane's cwd is already `$WB` (`-c`).
    let invocation_with_exit_code = format!(r#"{invocation}; printf '%s' "$?" > exit_code"#);
    // `pipe-pane` is chained onto the *same* tmux invocation as `new-session` via `\;` (tmux's own
    // multi-command separator, escaped so the outer shell passes it through literally) rather than
    // being a separate `;`-joined statement. `new-session -d` starts the agent immediately, so a
    // pipe attached by a later, separate `tmux pipe-pane` call misses everything the agent writes
    // in the gap between session creation and that second command running — the agent's opening
    // banner, initial plan, and any immediate startup crash, silently dropped from `agent.log`
    // (#289). Chaining within one invocation attaches the pipe to the pane tmux itself just
    // created, before the calling shell moves on, so there is no such window.
    //
    // Fail loudly if the session can't start — most likely a residual `$SESS` collision. Without
    // this guard a `duplicate session` error from tmux is swallowed by the `;`-join and the trigger
    // returns success while launching nothing (the silent no-op #411 hardens against). Mirror the
    // prompt-copy guard: record the reason in agent.log and on stderr, then exit non-zero.
    inner_stmts.push(format!(
        r#"tmux new-session -d -s "$SESS" -c "$WB" {} \; pipe-pane -o -t "$SESS" "cat >> \"$WB\"/agent.log" || {{ echo "moadim: failed to start tmux session $SESS (already exists?); aborting launch" | tee -a "$WB/agent.log" >&2; exit 1; }}"#,
        shell_quote(&invocation_with_exit_code)
    ));
    stmts.push(format!(
        r#"{{ {} ; }} >> "$WB/launch.log" 2>&1"#,
        inner_stmts.join("; ")
    ));
    stmts.join("; ")
}

#[cfg(test)]
#[path = "command_tests.rs"]
mod command_tests;

#[cfg(test)]
#[path = "command_bin_resolution_tests.rs"]
mod command_bin_resolution_tests;

#[cfg(test)]
#[path = "command_placeholder_tests.rs"]
mod command_placeholder_tests;
