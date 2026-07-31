//! Server-query commands (`cleanup`, `trigger`, `status`) split out of `cli/mod.rs` to stay under the
//! repo's per-file line gate: they report on/query the running server rather than managing its
//! lifecycle (start/stop/restart), which remain in `cli/mod.rs`.

use super::{
    bind_addr, http_request, http_request_json, http_request_with_body, is_running,
    liveness_exit_code, parse_freed_bytes, parse_removed_count, read_pid_file, wait_until,
    Duration,
};

/// Ask a running server to reap finished, expired routine run workbenches now, and print the count.
///
/// Runs the same sweep as the hourly background task instead of waiting for the next tick, via the
/// `/api/v1/routines/cleanup` route. Prints how many workbenches were removed, or a hint when no
/// server is up. With `json`, emits a single machine-readable object instead so the result can be
/// piped into scripts.
///
/// Returns the process exit code to surface: `0` when the server handled the sweep, and
/// [`crate::cli::EXIT_NOT_RUNNING`] when no server is running, so scripts can branch on `$?`.
pub fn cleanup(json: bool) -> anyhow::Result<i32> {
    match http_request_with_body("POST", "/api/v1/routines/cleanup") {
        Ok((200, body)) => {
            let removed = parse_removed_count(&body).unwrap_or(0);
            let freed_bytes = parse_freed_bytes(&body).unwrap_or(0);
            if json {
                println!("{}", cleanup_json(removed, freed_bytes, true));
            } else {
                let plural = if removed == 1 { "" } else { "es" };
                println!(
                    "cleanup removed {removed} workbench{plural} (freed {})",
                    humanize_bytes(freed_bytes)
                );
            }
            Ok(liveness_exit_code(true))
        }
        Ok((status, _)) => {
            anyhow::bail!("unexpected response from server: HTTP {status}");
        }
        Err(_) => {
            if json {
                println!("{}", cleanup_json(0, 0, false));
            } else {
                println!("moadim is not running");
            }
            Ok(liveness_exit_code(false))
        }
    }
}

/// Render a byte count as a short human-readable size using 1024-based units. Values under 1 KiB
/// are shown as a bare integer (`512 B`); larger values use one decimal place (`12.4 MB`). Caps at
/// TB so the unit table can't be indexed out of range.
pub(super) fn humanize_bytes(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    if bytes < 1024 {
        return format!("{bytes} B");
    }
    // Approximating a byte count for human display never needs exact precision above 2^52 —
    // any `moadim` workbench/log total that large would already be a many-petabyte anomaly, and
    // the rendered value is rounded to one decimal place anyway.
    #[allow(
        clippy::cast_precision_loss,
        reason = "human-readable size display, not an exact value"
    )]
    let mut size = bytes as f64;
    let mut unit = 0;
    while size >= 1024.0 && unit < UNITS.len() - 1 {
        size /= 1024.0;
        unit += 1;
    }
    format!("{size:.1} {}", UNITS[unit])
}

/// Ask a running server to trigger routine `id` immediately, outside its schedule, via the
/// `POST /routines/{id}/trigger` route — the same on-demand run the REST API and MCP tool already
/// expose, finally reachable from the terminal.
///
/// Prints a confirmation when the routine was triggered, an error when no routine has that id
/// (`404`), and a "not running" hint when no server is reachable. Returns the process exit code to
/// surface, mirroring the `status`/`cleanup` contract: `0` when the routine was triggered, and
/// [`crate::cli::EXIT_NOT_RUNNING`] when no server is running, so scripts can branch on `$?`.
pub fn trigger(id: &str) -> anyhow::Result<i32> {
    match http_request("POST", &format!("/api/v1/routines/{id}/trigger")) {
        Ok(200) => {
            println!("triggered routine {id}");
            Ok(liveness_exit_code(true))
        }
        Ok(404) => {
            anyhow::bail!("no routine with id {id}");
        }
        Ok(status) => {
            anyhow::bail!("unexpected response from server: HTTP {status}");
        }
        Err(_) => {
            println!("moadim is not running");
            Ok(liveness_exit_code(false))
        }
    }
}

/// Print a routine's newest run log (`agent.log`) to stdout via `GET /api/v1/routines/{id}/logs`
/// — the same route `moadim routines logs <id>` already drives, exposed here as a shorter
/// top-level alias (issue #332). An empty log (no run yet) prints nothing and exits `0`, matching
/// `svc_logs` returning an empty string. Uses the generous data-op timeout (like other data-plane
/// commands) rather than the liveness-probe one, since reading a large log is real work.
pub fn logs(id: &str) -> anyhow::Result<i32> {
    match http_request_json("GET", &format!("/api/v1/routines/{id}/logs"), None) {
        Ok((200, body)) => {
            if !body.is_empty() {
                println!("{body}");
            }
            Ok(liveness_exit_code(true))
        }
        Ok((404, _)) => {
            anyhow::bail!("no routine with id {id}");
        }
        Ok((status, _)) => {
            anyhow::bail!("unexpected response from server: HTTP {status}");
        }
        Err(_) => {
            println!("moadim is not running");
            Ok(liveness_exit_code(false))
        }
    }
}

/// Report whether a server is running, with its PID when known. With `json`, emits a single
/// machine-readable object instead of the human-readable line.
///
/// When `wait_secs` is `Some`, and no server answers on the first check, polls `GET /health`
/// every `WAIT_POLL_INTERVAL` until one does or the timeout elapses, so a caller can block on
/// startup (`moadim & moadim status --wait`) instead of sleeping blindly before probing.
///
/// Returns the process exit code to surface: `0` when a server is reachable, and
/// [`crate::cli::EXIT_NOT_RUNNING`] when not (including after a `--wait` timeout), so scripts can branch on
/// `$?` without parsing stdout.
pub fn status(json: bool, wait_secs: Option<u64>) -> anyhow::Result<i32> {
    let mut running = is_running();
    if !running {
        if let Some(secs) = wait_secs {
            running = wait_until(is_running, Duration::from_secs(secs));
        }
    }
    let pid = read_pid_file();
    if json {
        // Fold the server's own /health (uptime + version) into the object so a single
        // `status --json` answers liveness *and* age/version without a second call. When the
        // server is down (or answers unparseably) these fields are emitted as null.
        let health = if running { fetch_health() } else { None };
        println!("{}", status_json(running, pid, health.as_ref()));
        return Ok(liveness_exit_code(running));
    }
    if running {
        let pid_suffix = pid
            .map(|process_id| format!(" (pid {process_id})"))
            .unwrap_or_default();
        println!("moadim is running{pid_suffix} at http://{}", bind_addr());
        if let Some(health) = fetch_health() {
            print_crontab_sync_warning(&health);
        }
    } else {
        println!("moadim is not running");
    }
    Ok(liveness_exit_code(running))
}

/// Print an actionable warning when the daemon reports a stale OS crontab sync.
fn print_crontab_sync_warning(health: &HealthInfo) {
    let Some(sync) = health.crontab_sync.as_ref().filter(|sync| !sync.ok) else {
        return;
    };
    let error = sync.last_error.as_deref().unwrap_or("unknown error");
    println!("warning: routine OS crontab is stale: {error}");
    println!("recovery: {CRONTAB_SYNC_RECOVERY_HINT}");
}
include!("status_json.rs");
