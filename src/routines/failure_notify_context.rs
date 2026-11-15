/// Where a failed run happened and how to open it, shared by the command and webhook hooks so a
/// receiver in a shared alert channel can tell hosts apart and jump straight to the run.
struct RunContext {
    /// This host's machine name.
    machine: String,
    /// Numeric agent exit code; `None` for a watchdog timeout or an unknown outcome.
    exit_code: Option<i32>,
    /// Run Detail page URL in the web UI.
    run_url: String,
}

impl RunContext {
    fn new(routine_id: &str, run_id: &str, exit_code: Option<i32>) -> Self {
        Self {
            machine: crate::machine::current_machine(),
            exit_code,
            run_url: run_url(global_public_base_url().as_deref(), routine_id, run_id),
        }
    }
}

/// The global `[notifications]` table: the hook fields plus the global-only URL base.
#[derive(serde::Deserialize)]
struct NotificationsTable {
    #[serde(flatten)]
    hooks: FailureNotificationConfig,
    public_base_url: Option<String>,
}

fn read_global_toml() -> Option<GlobalNotificationsToml> {
    std::fs::read_to_string(crate::paths::notifications_toml_path())
        .ok()
        .and_then(|text| toml::from_str::<GlobalNotificationsToml>(&text).ok())
}

/// Build the Run Detail URL. `public_base_url` (from global `[notifications]`) wins for
/// reverse-proxy / tailnet setups; otherwise the URL points at the daemon's own bind address.
fn run_url(public_base_url: Option<&str>, routine_id: &str, run_id: &str) -> String {
    let base = public_base_url
        .map(str::trim)
        .filter(|base| !base.is_empty())
        .map_or_else(
            || format!("http://{}", crate::cli::bind_addr()),
            |base| base.trim_end_matches('/').to_string(),
        );
    format!("{base}/runs/{routine_id}/{run_id}")
}

fn global_public_base_url() -> Option<String> {
    let parsed = read_global_toml()?;
    parsed
        .notifications
        .and_then(|table| table.public_base_url)
        .or(parsed.public_base_url)
}
