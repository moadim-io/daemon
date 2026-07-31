
/// Server-sourced liveness details pulled from `GET /health` to enrich `status --json`.
#[derive(Debug, PartialEq, Eq)]
pub(super) struct HealthInfo {
    /// Seconds the server reports it has been up.
    pub(super) uptime_secs: u64,
    /// The daemon version the server reports.
    pub(super) version: String,
    /// Last OS crontab sync snapshot from `/health`.
    pub(super) crontab_sync: Option<CrontabSyncInfo>,
}

/// OS crontab sync health fields folded into `moadim status`.
#[derive(Debug, PartialEq, Eq)]
pub(super) struct CrontabSyncInfo {
    /// Whether the most recent crontab sync attempt succeeded.
    pub(super) ok: bool,
    /// Last sync error, when the most recent attempt failed.
    pub(super) last_error: Option<String>,
    /// Unix timestamp for the last sync error, when supplied by the daemon.
    pub(super) last_error_at: Option<u64>,
}

/// Operator guidance when macOS/TCC leaves the managed routines crontab stale.
pub(super) const CRONTAB_SYNC_RECOVERY_HINT: &str =
    "grant Full Disk Access to the moadim binary (or its launcher), then restart/update a routine to retry crontab sync";

/// Render the `status` result as a one-line JSON object:
/// `{"running":bool,"pid":N|null,"address":…,"uptime_secs":N|null,"version":S|null}`.
///
/// `pid` is `null` when no pid file is present (or the server is down). `uptime_secs`/`version`
/// carry the running server's self-reported `/health` details (via `health`), and are `null` when
/// no server answers or its `/health` body could not be parsed.
pub(super) fn status_json(running: bool, pid: Option<u32>, health: Option<&HealthInfo>) -> String {
    let uptime_secs = health.map(|info| info.uptime_secs);
    let version = health.map(|info| info.version.as_str());
    let crontab_sync = health.and_then(|info| info.crontab_sync.as_ref());
    let crontab_sync_json = crontab_sync.map(|info| {
        serde_json::json!({
            "ok": info.ok,
            "last_error": info.last_error,
            "last_error_at": info.last_error_at,
            "recovery_hint": (!info.ok).then_some(CRONTAB_SYNC_RECOVERY_HINT),
        })
    });
    serde_json::json!({
        "running": running,
        "pid": pid,
        "address": bind_addr(),
        "uptime_secs": uptime_secs,
        "version": version,
        "crontab_sync": crontab_sync_json,
    })
    .to_string()
}

/// Probe the running server's `GET /health` and return its uptime/version, or `None` when the
/// request fails, the status is not `200`, or the body is not the expected JSON shape.
pub(super) fn fetch_health() -> Option<HealthInfo> {
    let (status, body) = http_request_with_body("GET", "/api/v1/health").ok()?;
    (status == 200).then(|| parse_health(&body)).flatten()
}

/// Extract status details from a [`HealthResponse`](crate::routes::health::HealthResponse)
/// JSON body. Returns `None` if required liveness fields are missing or have the wrong type.
pub(super) fn parse_health(body: &str) -> Option<HealthInfo> {
    let value: serde_json::Value = serde_json::from_str(body).ok()?;
    let uptime_secs = value.get("uptime_secs")?.as_u64()?;
    let version = value.get("version")?.as_str()?.to_string();
    let crontab_sync = value.get("crontab_sync").and_then(parse_crontab_sync);
    Some(HealthInfo {
        uptime_secs,
        version,
        crontab_sync,
    })
}

/// Parse the optional `crontab_sync` health object, dropping it when malformed.
fn parse_crontab_sync(value: &serde_json::Value) -> Option<CrontabSyncInfo> {
    Some(CrontabSyncInfo {
        ok: value.get("ok")?.as_bool()?,
        last_error: value
            .get("last_error")
            .and_then(|error| error.as_str().map(ToString::to_string)),
        last_error_at: value.get("last_error_at").and_then(serde_json::Value::as_u64),
    })
}

/// Render the `cleanup` result as a one-line JSON object:
/// `{"running":bool,"removed":N,"freed_bytes":N,"address":…}`. `removed`/`freed_bytes` are `0` when
/// the server is not running (`running:false`). `address` is the effective bound [`bind_addr`] the
/// request was sent to, matching `status --json`/`stop --json`'s object shape so every `--json`
/// command surfaces the endpoint it talked to. The pre-existing `running`/`removed` keys are
/// preserved; `freed_bytes` is additive.
pub(super) fn cleanup_json(removed: usize, freed_bytes: u64, running: bool) -> String {
    serde_json::json!({
        "running": running,
        "removed": removed,
        "freed_bytes": freed_bytes,
        "address": bind_addr(),
    })
    .to_string()
}
