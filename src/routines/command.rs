//! Prompt composition, slug/shell helpers, and the single-line tmux launch command builder.

use crate::paths::routine_prompt_path;

use super::agents::AgentCommand;
use super::model::Routine;

/// Slugify `title` into a filesystem- and tmux-safe identifier.
///
/// Lowercases, replaces each run of non-alphanumeric characters with a single `-`, and trims
/// leading/trailing `-`. Returns `"routine"` if nothing usable remains.
pub(crate) fn slugify(title: &str) -> String {
    let mut out = String::new();
    let mut prev_dash = false;
    for c in title.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
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
pub(crate) fn compose_prompt(routine: &Routine) -> String {
    let mut s = String::from("# Workbench\n");
    s.push_str(
        "You are working in an empty directory. These repositories are relevant — clone any you need:\n",
    );
    for repo in &routine.repositories {
        match &repo.branch {
            Some(b) => s.push_str(&format!("- {} (branch {})\n", repo.repository, b)),
            None => s.push_str(&format!("- {}\n", repo.repository)),
        }
    }
    s.push_str("\n---\n");
    s.push_str(&routine.prompt);
    s.push('\n');
    s
}

/// Substitute `{workbench}`, `{prompt_file}`, and `{prompt}` placeholders in `s`.
///
/// `{prompt}` expands to a shell command substitution that reads `prompt.md` from the agent's
/// cwd (the workbench), so the full prompt is passed as a single argument to the agent process.
pub(crate) fn substitute(s: &str, workbench: &str, prompt_file: &str) -> String {
    s.replace("{workbench}", workbench)
        .replace("{prompt_file}", prompt_file)
        .replace("{prompt}", r#""$(cat prompt.md)""#)
}

/// Return the first directory on the daemon's `PATH` that contains an executable named `bin`.
fn bin_dir(bin: &str) -> Option<String> {
    let path = std::env::var("PATH").ok()?;
    path.split(':')
        .filter(|d| !d.is_empty())
        .find(|d| std::path::Path::new(d).join(bin).is_file())
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
        if let Some(d) = bin_dir(bin) {
            dirs.push(d);
        }
    }
    for d in [
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
        dirs.push(d);
    }
    let mut seen = std::collections::HashSet::new();
    dirs.retain(|d| seen.insert(d.clone()));
    dirs.join(":")
}

/// Wrap `s` in single quotes for safe inclusion in a POSIX shell command.
pub(crate) fn shell_quote(s: &str) -> String {
    let mut out = String::from("'");
    for c in s.chars() {
        if c == '\'' {
            out.push_str("'\\''");
        } else {
            out.push(c);
        }
    }
    out.push('\'');
    out
}

/// Build the single-line shell command that creates a workbench and launches the agent in tmux.
///
/// The agent's cwd is the workbench (via `tmux -c`), so `{prompt_file}` resolves to `prompt.md`,
/// `{workbench}` to `.`, and `{prompt}` to the prompt's contents passed as one argument. The prompt
/// reaches the agent as a process argument (not keystrokes), so there is no readiness race. The
/// command is `;`-joined (no newlines) so it fits one crontab line.
pub(crate) fn build_routine_command(routine: &Routine, agent: &AgentCommand) -> String {
    let slug = slugify(&routine.title);
    let prompt_path = routine_prompt_path(&slug)
        .to_string_lossy()
        .into_owned();

    let prompt_file_ref = "prompt.md";
    let workbench_ref = ".";

    let mut invocation = vec![agent.command.clone()];
    for a in &agent.args {
        invocation.push(substitute(a, workbench_ref, prompt_file_ref));
    }
    let invocation = invocation.join(" ");

    let mut stmts = vec![
        // cron runs with a minimal PATH (/usr/bin:/bin) that omits tmux/claude/npm dirs. Bake the
        // daemon's own PATH into the line so the agent tools resolve the same way they do for a
        // manual trigger (which inherits the daemon's environment).
        format!("export PATH={}", shell_quote(&cron_path(&agent.command))),
        r#"TS="$(date +%s)""#.to_string(),
        format!("SLUG={}", shell_quote(&slug)),
        r#"WB="$HOME/.moadim/workbenches/$SLUG-$TS""#.to_string(),
        r#"SESS="moadim-$SLUG-$TS""#.to_string(),
        r#"mkdir -p "$WB""#.to_string(),
        format!(r#"cp {} "$WB/prompt.md""#, shell_quote(&prompt_path)),
    ];
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
