/// Atomic file writes (write temp + rename) so readers never observe a torn file.
pub mod atomic;
/// Prune a reaped workbench's stale `projects` entry from the shared `~/.claude.json`.
pub mod claude_json;
/// Cron expression normalization and validation, shared by routine scheduling.
pub mod cron;
/// Owner-only (`0700`/`0600`) filesystem helpers for the daemon's secret-bearing tree.
pub mod fs_perms;
/// Poison-tolerant locking for the in-memory stores.
pub mod lock;
/// Spawn child processes and reap them so triggers don't leak zombie (`<defunct>`) entries.
pub mod process;
/// Startup print printed to stdout when the server begins listening.
pub mod startup_print;
/// Unix timestamp utilities.
pub mod time;
