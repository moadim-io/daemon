//! Retention (TTL) config for finished-run workbenches.
//!
//! A finished workbench is reaped once it is older than its routine's TTL (see the cleanup module).
//! A routine without an explicit `ttl_secs` falls back to [`DEFAULT_TTL_SECS`].

use super::super::model::Routine;

/// Default retention for a finished run's workbench when a routine sets no explicit `ttl_secs`.
pub const DEFAULT_TTL_SECS: u64 = 7 * 24 * 60 * 60;

impl Routine {
    /// Retention for this routine's finished workbenches: its `ttl_secs` or [`DEFAULT_TTL_SECS`].
    pub fn effective_ttl_secs(&self) -> u64 {
        self.ttl_secs.unwrap_or(DEFAULT_TTL_SECS)
    }
}
