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

/// Shell statements that write the agent's instructions file (e.g. `CLAUDE.md` for Claude,
/// `AGENTS.md` for Codex) into `$WB` with two layers:
///
/// 1. **Moadim prompt** — daemon-managed preamble plus a run-time date stamp.
/// 2. **User prompt** — contents of `~/.config/moadim/user_prompt.md`, appended if the file exists.
///
/// `instructions_file` is the workbench-relative filename the selected agent reads its project
/// instructions from; writing the daemon-managed preamble there guarantees the agent that actually
/// runs sees it.
///
/// Uses `printf '%b'` so `\n` sequences in the static header expand to real newlines without
/// embedding literal newlines in the crontab line. `$WB` must be in scope when the statements run.
pub(crate) fn system_prompt_stmts(
    user_prompt_path: &str,
    _routine_title: &str,
    instructions_file: &str,
) -> Vec<String> {
    let header = shell_quote(MOADIM_SYSTEM_PROMPT);
    let uq = shell_quote(user_prompt_path);
    let dest = format!(r#""$WB/{instructions_file}""#);
    vec![
        // Fail-fast if the daemon-managed instructions write fails. The statements are `;`-joined,
        // so a bare redirection failure (read-only/full $HOME, an unwritable $WB, disk-quota/inode
        // exhaustion) would be ignored and the agent would launch with no `CLAUDE.md` — hence no
        // moadim preamble. Abort instead, recording the reason in the workbench's agent.log
        // (already created via mkdir) and on stderr. Only this primary write is guarded; the
        // optional user-prompt append below stays best-effort (`|| true`).
        format!(
            r#"printf '%b\n\n**Run date**: %s\n**Timezone**: %s\n' {} "$(date)" "$(date +%Z)" > {dest} || {{ echo "moadim: failed to write agent instructions preamble; aborting launch" | tee -a "$WB/agent.log" >&2; exit 1; }}"#,
            header
        ),
        format!(
            r"[ -f {uq} ] && {{ printf '\n---\n\n'; cat {uq}; printf '\n'; }} >> {dest} || true",
            uq = uq
        ),
    ]
}
