//! Logging backend initialization. Defaults to `env_logger`'s human-readable format; set
//! `MOADIM_LOG_FORMAT=json` to emit one JSON object per line instead, for log aggregators
//! (Loki, ELK, Vector, CloudWatch) ingesting a detached daemon's `daemon.log`.

use std::io::Write;

#[cfg(test)]
#[path = "tests.rs"]
mod tests;

/// Output format selected by `MOADIM_LOG_FORMAT`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LogFormat {
    /// `env_logger`'s default human-readable output. The fallback for an unset or unrecognized
    /// `MOADIM_LOG_FORMAT`.
    Text,
    /// One JSON object per line: `ts`, `level`, `target`, `msg`.
    Json,
}

impl LogFormat {
    /// Unset or unrecognized values fall back to [`LogFormat::Text`] — the current human-readable
    /// output stays byte-for-byte the default with the variable unset.
    fn parse(value: Option<&str>) -> Self {
        match value {
            Some(value) if value.eq_ignore_ascii_case("json") => Self::Json,
            _ => Self::Text,
        }
    }
}

/// Render a log record as a single-line JSON object: `ts` (RFC 3339), `level`, `target`, `msg`.
fn format_json_line(record: &log::Record<'_>) -> String {
    serde_json::json!({
        "ts": chrono::Utc::now().to_rfc3339(),
        "level": record.level().to_string(),
        "target": record.target(),
        "msg": record.args().to_string(),
    })
    .to_string()
}

/// Initialize the `log` backend so `log::*` call sites across the daemon actually emit; without
/// an installed backend the `log` facade is a silent no-op. Defaults to the `info` level and is
/// overridable via `RUST_LOG` in both formats. Uses `try_init` to avoid panicking if a backend is
/// already installed (e.g. under repeated test setup).
pub fn init() {
    let mut builder =
        env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"));
    if LogFormat::parse(std::env::var("MOADIM_LOG_FORMAT").ok().as_deref()) == LogFormat::Json {
        builder.format(|buf, record| writeln!(buf, "{}", format_json_line(record)));
    }
    let _ = builder.try_init();
}
