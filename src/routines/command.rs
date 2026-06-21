//! Prompt composition, slug/shell helpers, and the single-line tmux launch command builder.

use crate::paths::{routine_prompt_path, routine_scheduled_state_path};

use super::agents::AgentCommand;
use super::model::Routine;

/// Slugify `title` into a filesystem- and tmux-safe identifier.
///
/// Lowercases, replaces each run of non-alphanumeric characters with a single `-`, and trims
/// leading/trailing `-`. Returns `"routine"` if nothing usable remains.
pub(crate) fn slugify(title: &str) -> String {
    let mut out = String::new();
    let mut prev_dash = false;
    for ch in title.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
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

/// Compose the `prompt.md` body: a repositories-as-context preamble followed by the prompt.
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
    body.push_str("\n---\n");
    body.push_str(&routine.prompt);
    body.push('\n');
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

/// Return the first directory on the daemon's `PATH` that contains an executable named `bin`.
fn bin_dir(bin: &str) -> Option<String> {
    let path = std::env::var("PATH").ok()?;
    path.split(':')
        .filter(|dir| !dir.is_empty())
        .find(|dir| std::path::Path::new(dir).join(bin).is_file())
        .map(str::to_string)
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

/// Shell statements that write `CLAUDE.md` into `$WB` with two layers:
///
/// 1. **Moadim prompt** — daemon-managed preamble, the routine-origin disclosure naming
///    `routine_title`, plus a run-time date stamp.
/// 2. **User prompt** — contents of `~/.config/moadim/user_prompt.md`, appended if the file exists.
///
/// Uses `printf '%b'` so `\n` sequences in the static header expand to real newlines without
/// embedding literal newlines in the crontab line. `$WB` must be in scope when the statements run.
pub(crate) fn system_prompt_stmts(user_prompt_path: &str, routine_title: &str) -> Vec<String> {
    let header = shell_quote(MOADIM_SYSTEM_PROMPT);
    let disclosure = shell_quote(MOADIM_DISCLOSURE);
    let title = shell_quote(routine_title);
    let uq = shell_quote(user_prompt_path);
    vec![
        format!(
            r#"printf '%b\n\n%b%s\n\n**Run date**: %s\n**Timezone**: %s\n' {} {} {} "$(date)" "$(date +%Z)" > "$WB/CLAUDE.md""#,
            header, disclosure, title
        ),
        format!(
            r#"[ -f {uq} ] && {{ printf '\n---\n\n'; cat {uq}; printf '\n'; }} >> "$WB/CLAUDE.md" || true"#,
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
    let prompt_path = routine_prompt_path(&slug).to_string_lossy().into_owned();
    let scheduled_state_path = routine_scheduled_state_path(&slug)
        .to_string_lossy()
        .into_owned();

    let prompt_file_ref = "prompt.md";
    let workbench_ref = ".";

    let mut invocation = vec![agent.command.clone()];
    for arg in &agent.args {
        invocation.push(substitute(arg, workbench_ref, prompt_file_ref));
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
        // Record this scheduled firing. The daemon never sees a cron run (the OS crontab executes
        // this script directly), so the script itself stamps the fire time into the routine's
        // gitignored `scheduled.local.toml` sidecar; the daemon reads it back into
        // `last_scheduled_trigger_at` on load. Written before the prompt-copy guard below so an
        // aborted run still records that the schedule fired, and best-effort (`|| true`) so a
        // sidecar write failure never blocks launching the agent.
        format!(
            r#"printf 'last_scheduled_trigger_at = %s\n' "$TS" > {} || true"#,
            shell_quote(&scheduled_state_path)
        ),
        format!("SLUG={}", shell_quote(&slug)),
        r#"WB="$HOME/.moadim/workbenches/$SLUG-$TS""#.to_string(),
        r#"SESS="moadim-$SLUG-$TS""#.to_string(),
        r#"mkdir -p "$WB""#.to_string(),
    ];
    stmts.extend(system_prompt_stmts(
        &crate::paths::user_prompt_path().to_string_lossy(),
        &routine.title,
    ));
    stmts.extend([
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
        stmts.push(setup.clone());
    }
    stmts.push(format!(
        r#"tmux new-session -d -s "$SESS" -c "$WB" {}"#,
        shell_quote(&invocation)
    ));
    stmts.push(r#"tmux pipe-pane -o -t "$SESS" "cat >> \"$WB\"/agent.log""#.to_string());
    stmts.join("; ")
}

#[cfg(test)]
#[path = "command_tests.rs"]
mod command_tests;
