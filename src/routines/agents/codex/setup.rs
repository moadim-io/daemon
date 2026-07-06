//! Built-in default agent config for Codex.

/// Registry key for this agent; also the config filename stem (`codex.toml`).
pub const NAME: &str = "codex";

/// Default `codex.toml` contents, written on startup when the file is absent.
///
/// `codex exec` is already non-interactive (no approval prompts), but its default `workspace-write`
/// sandbox disables outbound network access — so an unattended routine could not clone the remote
/// repo or push / open a PR. The default therefore re-enables network while keeping writes scoped
/// to the workbench (least privilege), mirroring the "launches unattended, with the access the task
/// needs" baseline the `claude` default already gets.
///
/// Codex reads its project instructions from `AGENTS.md`, not Claude Code's `CLAUDE.md`, so the
/// daemon must write the moadim system prompt + routine-origin disclosure there for it to be seen.
pub const CONFIG: &str = r#"command = "codex"
# `codex exec` runs unattended, but its default workspace-write sandbox blocks the
# network, so a routine could not clone the repo or push / open a PR. Pin the sandbox
# to workspace-write explicitly (so a future default change can't silently widen or
# narrow it) and turn network access back on — the least-privilege setting that still
# lets the routine reach the remote. Override in ~/.config/moadim/agents/codex.toml:
# drop the `-c` line to re-disable network, or swap in
# `--dangerously-bypass-approvals-and-sandbox` for unrestricted access.
args = ["exec", "-s", "workspace-write", "-c", "sandbox_workspace_write.network_access=true", "{prompt_file}"]
instructions_file = "AGENTS.md"
"#;
