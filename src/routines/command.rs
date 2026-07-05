//! Prompt composition, slug/shell helpers, and the single-line tmux launch command builder.

use crate::paths::{routine_compiled_prompt_path, routine_scheduled_log_path};

use super::agents::AgentCommand;
use super::flags::{list_flags, FlagScope};
use super::model::Routine;

/// Slugify `title` into a filesystem- and tmux-safe identifier.
///
/// Lowercases, replaces each run of non-alphanumeric characters with a single `-`, and trims
/// leading/trailing `-`. Returns `"routine"` if nothing usable remains.
///
/// Unicode-aware: uses [`char::is_alphanumeric`] / [`char::to_lowercase`] rather than the ASCII-only
/// variants, so non-Latin titles (Hebrew, CJK, Cyrillic) and Latin letters with diacritics (`é`,
/// `ü`) keep their content instead of collapsing to the `"routine"` fallback (#262). Both the
/// on-disk workbench dir and the tmux session name are shell-quoted wherever the slug is embedded,
/// so non-ASCII bytes there are safe.
pub(crate) fn slugify(title: &str) -> String {
    let mut out = String::new();
    let mut prev_dash = false;
    for ch in title.chars() {
        if ch.is_alphanumeric() {
            out.extend(ch.to_lowercase());
            prev_dash = false;
        } else if !prev_dash {
            out.push('-');
            prev_dash = true;
        }
    }
    let trimmed = out.trim_matches('-').to_string();
    if trimmed.is_empty() {
        "routine".to_string()
    } else {
        trimmed
    }
}

/// Compose the `prompt.compiled.md` body: a repositories-as-context preamble, an optional `## Goal`
/// section, the prompt, and — when the routine has any — an "Open flags" section listing
/// gaps/bugs/edge cases the agent raised on a previous run (see [`super::flags`]) that no one has
/// resolved yet.
///
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
            match &repo.branch {
                Some(branch) => {
                    body.push_str(&format!("- {} (branch {})\n", repo.repository, branch));
                }
                None => body.push_str(&format!("- {}\n", repo.repository)),
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
            body.push_str(&format!(
                "- **{}** ({scope}): {}\n",
                flag.flag_type, flag.description
            ));
        }
    }
    body
}

/// Substitute `{workbench}`, `{prompt_file}`, and `{prompt}` placeholders in `s`.
///
/// `{prompt}` expands to a shell command substitution that reads `prompt.md` from the agent's
/// cwd (the workbench), so the full prompt is passed as a single argument to the agent process.
pub(crate) fn substitute(template: &str, workbench: &str, prompt_file: &str) -> String {
    template
        .replace("{workbench}", workbench)
        .replace("{prompt_file}", prompt_file)
        .replace("{prompt}", r#""$(cat prompt.md)""#)
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

/// Return the first directory on the daemon's `PATH` that contains an executable named `bin`.
fn bin_dir(bin: &str) -> Option<String> {
    let path = std::env::var("PATH").ok()?;
    bin_dir_in(&path, bin)
}

/// Return the first directory in the `:`-separated `path` list that contains a file named `bin`.
///
/// Split out from [`bin_dir`] so the resolution logic is injectable in tests: callers can point
/// `path` at a temp dir with or without a fake binary without mutating the process-global `PATH`.
fn bin_dir_in(path: &str, bin: &str) -> Option<String> {
    path.split(':')
        .filter(|dir| !dir.is_empty())
        .find(|dir| std::path::Path::new(dir).join(bin).is_file())
        .map(str::to_string)
}

/// Whether `tmux` resolves to a file on the given `:`-separated `path` list.
///
/// `tmux` is a hard runtime dependency: routine launches run `tmux new-session …; tmux pipe-pane …`
/// and a missing `tmux` would be silently ignored (the statements are `;`-joined), making the run a
/// no-op. This helper surfaces its presence so startup can warn and `GET /health` can report it.
/// Injectable for tests via the `path` argument; see [`tmux_available`] for the live-`PATH` variant.
pub(crate) fn tmux_available_in(path: &str) -> bool {
    bin_dir_in(path, "tmux").is_some()
}

/// Whether `tmux` resolves on the daemon's live `PATH`. Returns `false` when `PATH` is unset.
pub(crate) fn tmux_available() -> bool {
    std::env::var("PATH")
        .ok()
        .is_some_and(|path| tmux_available_in(&path))
}

/// Whether `command` resolves to a file on the given `:`-separated `path` list.
///
/// Generalizes [`tmux_available_in`] to an arbitrary executable name: a routine's agent `command`
/// (e.g. `claude`, `codex`) is launched the same way `tmux` is — unresolved, it makes the cron
/// firing a silent no-op. Used to distinguish "agent config present" from "agent binary actually
/// runnable" in [`super::model::RoutineResponse`]. Injectable for tests via the `path` argument;
/// see [`agent_command_available`] for the live-`PATH` variant.
pub(crate) fn agent_command_available_in(path: &str, command: &str) -> bool {
    bin_dir_in(path, command).is_some()
}

/// Whether `command` resolves on the daemon's live `PATH`. Returns `false` when `PATH` is unset.
pub(crate) fn agent_command_available(command: &str) -> bool {
    std::env::var("PATH")
        .ok()
        .is_some_and(|path| agent_command_available_in(&path, command))
}

/// Common install locations to probe for `tmux` when it is not on `path` at all.
///
/// Split out so [`resolve_tmux_bin_from`] can be exercised in tests against fake, temp-dir-anchored
/// fallback lists instead of these real absolute paths (which may or may not hold a real `tmux` on
/// the machine running the tests).
fn tmux_fallback_dirs(home: &str) -> Vec<String> {
    vec![
        "/opt/homebrew/bin".to_string(),
        "/usr/local/bin".to_string(),
        format!("{home}/.local/bin"),
    ]
}

/// Best-effort absolute path to `tmux`: first dir on `path` holding it, else the first of
/// `fallback_dirs` holding it, else the bare `"tmux"` name.
///
/// Injectable variant of [`resolve_tmux_bin`] for tests. Mirrors the fallback list [`cron_path`]
/// bakes into crontab lines: launchd/systemd start the daemon with a minimal `PATH`
/// (`/usr/bin:/bin:/usr/sbin:/sbin`) that hides a Homebrew- or npm-installed `tmux`, so the
/// daemon's own tmux probes (`routines::cleanup::session`) would otherwise always fail to find it
/// — every liveness check then reads as "not running", so a hung run's workbench gets TTL-reaped
/// while the real tmux session and agent process are never killed and become permanently
/// untracked. Returning the bare `"tmux"` name when it cannot be found anywhere leaves the
/// caller's `Command::new` failing exactly as before.
fn resolve_tmux_bin_from(path: &str, fallback_dirs: &[String]) -> String {
    if let Some(dir) = bin_dir_in(path, "tmux") {
        return format!("{dir}/tmux");
    }
    for dir in fallback_dirs {
        if std::path::Path::new(dir).join("tmux").is_file() {
            return format!("{dir}/tmux");
        }
    }
    "tmux".to_string()
}

/// Live-`PATH`/`HOME` variant of [`resolve_tmux_bin_from`]; see its docs for why this exists.
pub(crate) fn resolve_tmux_bin() -> String {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
    let path = std::env::var("PATH").unwrap_or_default();
    resolve_tmux_bin_from(&path, &tmux_fallback_dirs(&home))
}

/// A short `PATH` for cron, since cron's default (`/usr/bin:/bin`) hides homebrew/npm-installed
/// tools like `tmux` and the agent binary.
///
/// Baking the daemon's full inherited `PATH` is not viable: it can exceed cron's per-line length
/// limit (~1000 chars) and silently disable the job. Instead this resolves just the dirs holding
/// `tmux` and the agent `command`, then appends common tool locations and the cron defaults,
/// deduplicated and order-preserving — short enough to stay well under the limit.
fn cron_path(agent_command: &str) -> String {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
    let mut dirs: Vec<String> = Vec::new();
    for bin in ["tmux", agent_command] {
        if let Some(dir) = bin_dir(bin) {
            dirs.push(dir);
        }
    }
    for dir in [
        format!("{home}/.local/bin"),
        "/opt/homebrew/bin".to_string(),
        "/usr/local/bin".to_string(),
        format!("{home}/.cargo/bin"),
        format!("{home}/.bun/bin"),
        "/usr/bin".to_string(),
        "/bin".to_string(),
        "/usr/sbin".to_string(),
        "/sbin".to_string(),
    ] {
        dirs.push(dir);
    }
    let mut seen = std::collections::HashSet::new();
    dirs.retain(|dir| seen.insert(dir.clone()));
    dirs.join(":")
}

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

/// Moadim-managed preamble written to every workbench `CLAUDE.md`.
///
/// Uses `\n` as literal two-character sequences (not real newlines) so the text can be embedded
/// in a single crontab line and passed to `printf '%b'`, which re-expands them into newlines at
/// run time. The run date and timezone are appended dynamically by the shell.
const MOADIM_SYSTEM_PROMPT: &str = "# Moadim Context\\n\
    \\n\
    > This section is managed by the moadim daemon. Do not edit it.\\n\
    \\n\
    You are running inside a moadim-managed agent session. \
    Complete the task described in `prompt.md` and exit when done.";

/// Routine-origin disclosure appended to the moadim system prompt.
///
/// Instructs the agent to reveal, in every outward-facing communication, that it acts on behalf of
/// the moadim routine — naming it. The routine name itself is *not* part of this constant: it is
/// injected at run time as a separate `printf` `%s` argument (text expanded by `printf '%b'` is not
/// re-scanned for conversions, so a `%s` placed inside this `%b` string would print literally).
/// This constant therefore ends just before the name. `\n` are literal two-character sequences
/// re-expanded into newlines by `printf '%b'`, matching `MOADIM_SYSTEM_PROMPT`.
const MOADIM_DISCLOSURE: &str = "## Routine origin disclosure\\n\
    \\n\
    You act on behalf of the moadim routine named below. In every external, outward-facing \
    communication you produce — GitHub issues, pull requests and comments; Slack messages; emails; \
    any channel a human or third-party system receives — you MUST disclose that the action \
    originates from this moadim routine, naming it (for example: 'This pull request was opened by \
    the <routine name> routine of moadim.'). Phrasing may be adapted per channel but must \
    include the routine name. This does NOT apply to internal logs or in-repo working files.\\n\
    \\n\
    Routine name: ";

/// Shell statements that write the agent's instructions file (e.g. `CLAUDE.md` for Claude,
/// `AGENTS.md` for Codex) into `$WB` with two layers:
///
/// 1. **Moadim prompt** — daemon-managed preamble, the routine-origin disclosure naming
///    `routine_title`, plus a run-time date stamp.
/// 2. **User prompt** — contents of `~/.config/moadim/user_prompt.md`, appended if the file exists.
///
/// `instructions_file` is the workbench-relative filename the selected agent reads its project
/// instructions from; writing the disclosure there guarantees the agent that actually runs sees it.
///
/// Uses `printf '%b'` so `\n` sequences in the static header expand to real newlines without
/// embedding literal newlines in the crontab line. `$WB` must be in scope when the statements run.
pub(crate) fn system_prompt_stmts(
    user_prompt_path: &str,
    routine_title: &str,
    instructions_file: &str,
) -> Vec<String> {
    let header = shell_quote(MOADIM_SYSTEM_PROMPT);
    let disclosure = shell_quote(MOADIM_DISCLOSURE);
    let title = shell_quote(routine_title);
    let uq = shell_quote(user_prompt_path);
    let dest = format!(r#""$WB/{instructions_file}""#);
    vec![
        // Fail-fast if the disclosure write fails. The statements are `;`-joined, so a bare
        // redirection failure (read-only/full $HOME, an unwritable $WB, disk-quota/inode
        // exhaustion) would be ignored and the agent would launch with no `CLAUDE.md` — hence no
        // routine-origin disclosure mandate, the central transparency guarantee of this project.
        // Abort instead, mirroring the `cp prompt.md` guard below: record the reason in the
        // workbench's agent.log (already created via mkdir) and on stderr. Only this primary write
        // is guarded; the optional user-prompt append below stays best-effort (`|| true`).
        format!(
            r#"printf '%b\n\n%b%s\n\n**Run date**: %s\n**Timezone**: %s\n' {} {} {} "$(date)" "$(date +%Z)" > {dest} || {{ echo "moadim: failed to write agent instructions disclosure; aborting launch" | tee -a "$WB/agent.log" >&2; exit 1; }}"#,
            header, disclosure, title
        ),
        format!(
            r#"[ -f {uq} ] && {{ printf '\n---\n\n'; cat {uq}; printf '\n'; }} >> {dest} || true"#,
            uq = uq
        ),
    ]
}

/// Build the single-line shell command that creates a workbench and launches the agent in tmux.
///
/// The agent's cwd is the workbench (via `tmux -c`), so `{prompt_file}` resolves to `prompt.md`,
/// `{workbench}` to `.`, and `{prompt}` to the prompt's contents passed as one argument. The prompt
/// reaches the agent as a process argument (not keystrokes), so there is no readiness race. The
/// command is `;`-joined (no newlines) so it fits one crontab line.
pub(crate) fn build_routine_command(routine: &Routine, agent: &AgentCommand) -> String {
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
        // The crontab invokes this script under a *login* shell (`/bin/sh -l`; see
        // `sync::routines::format_routine_line`), so the user's `~/.profile` is sourced first and
        // the agent inherits their environment — GH_TOKEN, API keys and the like — which cron's
        // minimal env (and, on macOS, the GUI-Keychain-less session) otherwise withholds.
        //
        // PATH is still *replaced* with this curated list (not merged with the profile's), keeping
        // binary resolution identical to before the login-shell change: tmux and the agent always
        // resolve to the same dirs the daemon itself uses, regardless of how the profile orders
        // PATH. Only environment *variables* are gained from the profile; PATH behaviour is
        // unchanged.
        format!("export PATH={}", shell_quote(&cron_path(&agent.command))),
        r#"TS="$(date +%s)""#.to_string(),
        // Record this scheduled firing. Appends the Unix timestamp as one line to the routine's
        // gitignored `scheduled.log`; the daemon reads the last line back as
        // `last_scheduled_trigger_at` on load. Using `>>` (append) preserves the full run history.
        // Written before the prompt-copy guard below so an aborted run still records that the
        // schedule fired, and best-effort (`|| true`) so a log write failure never blocks launching.
        format!(
            r#"printf '%s\n' "$TS" >> {} || true"#,
            shell_quote(&scheduled_log_path)
        ),
        format!("SLUG={}", shell_quote(&slug)),
        format!(r#"WB={}/"$SLUG-$TS""#, shell_quote(&workbenches_base)),
        r#"SESS="moadim-$SLUG-$TS""#.to_string(),
        r#"mkdir -p "$WB""#.to_string(),
    ];

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
        // Inserted verbatim so the agent author controls quoting; `$WB`/`$SESS` are in scope.
        inner_stmts.push(setup.clone());
    }
    // Record the agent's exit code once it finishes, so the run-history view (`svc_list_runs`)
    // can tell success from failure instead of only "session ended". `tmux new-session` runs a
    // single quoted string through the pane's default shell, so `;`-appending here shares that
    // same shell and its `$?`. Written to a workbench-*relative* path (`exit_code`, not
    // `$WB/exit_code`): `$WB` is a plain (non-exported) shell variable in the launcher script and
    // is not inherited by the new shell tmux spawns, but the pane's cwd is already `$WB` (`-c`).
    let invocation_with_exit_code = format!(r#"{invocation}; printf '%s' "$?" > exit_code"#);
    inner_stmts.push(format!(
        r#"tmux new-session -d -s "$SESS" -c "$WB" {}"#,
        shell_quote(&invocation_with_exit_code)
    ));
    inner_stmts.push(r#"tmux pipe-pane -o -t "$SESS" "cat >> \"$WB\"/agent.log""#.to_string());
    stmts.push(format!(
        r#"{{ {} ; }} >> "$WB/launch.log" 2>&1"#,
        inner_stmts.join("; ")
    ));
    stmts.join("; ")
}

#[cfg(test)]
#[path = "command_tests.rs"]
mod command_tests;
