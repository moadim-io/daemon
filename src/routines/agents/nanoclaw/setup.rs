//! Built-in default agent config for `NanoClaw`.

/// Registry key for this agent; also the config filename stem (`nanoclaw.toml`).
pub const NAME: &str = "nanoclaw";

/// Default `nanoclaw.toml` contents, written on startup when the file is absent.
///
/// `NanoClaw` accepts host-created work through `ncl tasks create`, rather than a direct one-shot
/// agent command. This bridge queues a one-shot task for the configured `NanoClaw` agent group; the
/// `NanoClaw` host owns execution and delivery after `ncl` accepts the task.
pub const CONFIG: &str = r#"command = "sh"
args = ["-c", "exec ncl tasks create --group \"${NANOCLAW_AGENT_GROUP_ID:?set NANOCLAW_AGENT_GROUP_ID}\" --name moadim --process-after \"$(date -u +%Y-%m-%dT%H:%M:%SZ)\" --prompt \"$(cat {prompt_file})\""]
"#;
