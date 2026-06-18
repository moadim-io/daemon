/// Atomic file writes (write temp + rename) so readers never observe a torn file.
pub mod atomic;
/// Spawn child processes fire-and-forget while reaping them to avoid zombies.
pub mod process;
/// JSON Schema helpers for schemars.
pub mod schema;
/// Startup print printed to stdout when the server begins listening.
pub mod startup_print;
/// Unix timestamp utilities.
pub mod time;
