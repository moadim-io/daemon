//! Offline `moadim export` / `moadim import`: back up and restore the tracked moadim config.
//!
//! `moadim export` snapshots every *tracked* config file — per-routine `routine.toml`,
//! `schedule.cron`, `disabled.json`, and `prompts/prompt.pure.md`, the agent registry's
//! `agents/*.toml`, plus the global `notifications.toml` and `user_prompt.md` — into a single
//! portable JSON bundle ([`bundle::Bundle`]). Gitignored runtime sidecars (`*.local.*`, `*.pid`,
//! `*.log`, compiled cron/prompt outputs) are excluded, mirroring the tracked-vs-local split the
//! config `.gitignore` already encodes, so secrets and machine-local state never leak into a
//! bundle.
//!
//! `moadim import` restores such a bundle into the config tree through the same atomic-write
//! primitive the storage layer uses ([`crate::utils::atomic::atomic_write`]), skipping files that
//! already exist unless `--force` is given, and validating every path and `routine.toml` payload
//! before touching disk. Both commands run offline against the filesystem — no daemon required —
//! so they work for backup/migration even when the server is stopped (issue #368).

/// The portable bundle format and the tracked-path predicate shared by export and import.
mod bundle;
/// `moadim export`: walk the config tree and serialize the tracked files into a bundle.
mod export;
/// `moadim import`: validate a bundle and restore its files into the config tree.
mod import;

pub(crate) use export::run_export;
pub(crate) use import::run_import;
