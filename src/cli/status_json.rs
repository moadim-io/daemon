
/// Server-sourced liveness details pulled from `GET /health` to enrich `status --json`.
#[derive(Debug, PartialEq, Eq)]
pub(super) struct HealthInfo {
    /// Seconds the server reports it has been up.
    pub(super) uptime_secs: u64,
    /// The daemon version the server reports.
    pub(super) version: String,
}

/// Render the `status` result as a one-line JSON object:
/// `{"running":bool,"pid":N|null,"address":…,"uptime_secs":N|null,"version":S|null}`.
///
/// `pid` is `null` when no pid file is present (or the server is down). `uptime_secs`/`version`
/// carry the running server's self-reported `/health` details (via `health`), and are `null` when
/// no server answers or its `/health` body could not be parsed.
pub(super) fn status_json(running: bool, pid: Option<u32>, health: Option<&HealthInfo>) -> String {
    let uptime_secs = health.map(|info| info.uptime_secs);
    let version = health.map(|info| info.version.as_str());
    serde_json::json!({
        "running": running,
        "pid": pid,
        "address": bind_addr(),
        "uptime_secs": uptime_secs,
        "version": version,
    })
    .to_string()
}

/// Probe the running server's `GET /health` and return its uptime/version, or `None` when the
/// request fails, the status is not `200`, or the body is not the expected JSON shape.
pub(super) fn fetch_health() -> Option<HealthInfo> {
    let (status, body) = http_request_with_body("GET", "/api/v1/health").ok()?;
    (status == 200).then(|| parse_health(&body)).flatten()
}

/// Extract `uptime_secs` and `version` from a [`HealthResponse`](crate::routes::health::HealthResponse)
/// JSON body. Returns `None` if either field is missing or the wrong type.
pub(super) fn parse_health(body: &str) -> Option<HealthInfo> {
    let value: serde_json::Value = serde_json::from_str(body).ok()?;
    let uptime_secs = value.get("uptime_secs")?.as_u64()?;
    let version = value.get("version")?.as_str()?.to_string();
    Some(HealthInfo {
        uptime_secs,
        version,
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
