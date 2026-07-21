//! Prompt composition, `{placeholder}` substitution/validation, and the inline-prompt size guard,
//! split out of `command.rs` to keep that file under the line-count gate.

use std::fmt::Write as _;

use super::slugify;
use crate::routines::agents::AgentCommand;
use crate::routines::flags::{list_flags, FlagScope};
use crate::routines::model::Routine;

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
pub(crate) fn placeholder_tokens(arg: &str) -> Vec<String> {
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
