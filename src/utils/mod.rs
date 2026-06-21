/// Atomic file writes (write temp + rename) so readers never observe a torn file.
pub mod atomic;
/// Owner-only (`0700`/`0600`) filesystem helpers for the daemon's secret-bearing tree.
pub mod fs_perms;
/// Poison-tolerant locking for the in-memory stores.
pub mod lock;
/// JSON Schema helpers for schemars.
pub mod schema;
/// Startup print printed to stdout when the server begins listening.
pub mod startup_print;
/// Unix timestamp utilities.
pub mod time;
