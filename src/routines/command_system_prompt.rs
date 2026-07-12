//! The moadim-managed system prompt written into every workbench's agent instructions file
//! (`CLAUDE.md`/`AGENTS.md`), split out of `command.rs` to keep that file under the line-count gate.

use super::shell_quote;

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
    Complete the task described in `prompt.md` and exit when done.\\n\
    \\n\
    ## Work log\\n\
    \\n\
    As you work, append short progress notes to `summary.md` in this workbench (create it if \
    it doesn't exist) — what you're doing and why, each time you start a new step. Before you \
    exit, write a `## Final summary` section to `summary.md` describing what was accomplished, \
    what changed, and anything left unresolved.";

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
            r"[ -f {uq} ] && {{ printf '\n---\n\n'; cat {uq}; printf '\n'; }} >> {dest} || true",
            uq = uq
        ),
    ]
}
