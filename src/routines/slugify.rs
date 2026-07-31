#![allow(
    clippy::wildcard_imports,
    clippy::too_many_arguments,
    clippy::needless_pass_by_value,
    dead_code,
    reason = "split-out module preserves existing code while keeping files under the linecheck limit"
)]
use super::*;

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
pub(crate) fn env_export_stmts(routine: &Routine) -> Vec<String> {
    let rel_dir = crate::routine_storage::routine_rel_dir(routine);
    let local_env = read_local_env(&rel_dir);
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
