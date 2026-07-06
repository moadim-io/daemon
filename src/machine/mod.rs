//! Machine identity for multi-machine deployments.
//!
//! One `~/.config/moadim` config repo can be shared (via the user's own git workflow) across several
//! machines — a laptop, a work box, a server. Each routine declares which machines run it through a
//! `machines` targeting list; each daemon then filters its crontab sync to only the entries naming
//! *this* machine. This module answers "which machine am I?".
//!
//! Identity resolves in priority order:
//! 1. the `MOADIM_MACHINE` environment variable (trimmed, non-empty),
//! 2. the `name` field in the gitignored `~/.config/moadim/machine.local.toml`,
//! 3. the system hostname.
//!
//! The file and env override exist because hostnames are not always meaningful or stable; the file
//! is `*.local.*` (gitignored) so a name set on one host never travels in the shared repo.

use serde::{Deserialize, Serialize};

use crate::error::AppError;
use crate::paths::machine_config_path;
use crate::utils::atomic::atomic_write;
use crate::utils::lock::LockRecover;

/// On-disk shape of `machine.local.toml`.
#[derive(Debug, Default, Deserialize, Serialize)]
struct MachineToml {
    /// This machine's identity name, matched against routine/job `machines` lists.
    name: Option<String>,
}

/// Where a resolved machine identity came from, for `moadim machine show` to report.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MachineSource {
    /// From the `MOADIM_MACHINE` environment variable.
    Env,
    /// From the `name` field in `machine.local.toml`.
    File,
    /// Auto-generated on first run and written to `machine.local.toml`.
    Generated,
    /// Fell back to the system hostname (only when writing the generated name fails).
    Hostname,
}

impl MachineSource {
    /// Short human label used in CLI output.
    pub fn label(self) -> &'static str {
        match self {
            Self::Env => "MOADIM_MACHINE env",
            Self::File => "machine.local.toml",
            Self::Generated => "auto-generated (first run)",
            Self::Hostname => "system hostname",
        }
    }
}

/// This machine's identity name (just the name, dropping the source).
pub fn current_machine() -> String {
    resolve().0
}

/// This machine's identity name together with where it was resolved from.
pub fn resolve() -> (String, MachineSource) {
    let env = std::env::var("MOADIM_MACHINE").ok();
    let file = read_machine_file();
    if let Some(name) = non_empty(env) {
        return (name, MachineSource::Env);
    }
    if let Some(name) = non_empty(file) {
        return (name, MachineSource::File);
    }
    // No name configured: generate a unique name and persist it so every subsequent
    // call returns the same identity without re-generating.
    let generated = generate_name();
    match set_machine(&generated) {
        Ok(()) => {
            log::warn!(
                "no machine name configured; generated {generated:?} — run `moadim machine set <name>` to choose your own"
            );
            (generated, MachineSource::Generated)
        }
        Err(err) => {
            log::warn!("failed to save generated machine name: {err}; falling back to hostname");
            (hostname(), MachineSource::Hostname)
        }
    }
}

/// Generate a unique machine name of the form `machine-{8hex}`.
fn generate_name() -> String {
    format!(
        "machine-{}",
        &uuid::Uuid::new_v4().simple().to_string()[..8]
    )
}

/// Pure resolution core: pick the first non-empty of env, then file, then hostname.
///
/// Split out from [`resolve`] so the precedence (and each branch) is unit-testable without touching
/// the real environment or filesystem.
#[cfg(test)]
fn resolve_from(
    env: Option<String>,
    file: Option<String>,
    hostname: String,
) -> (String, MachineSource) {
    if let Some(name) = non_empty(env) {
        return (name, MachineSource::Env);
    }
    if let Some(name) = non_empty(file) {
        return (name, MachineSource::File);
    }
    (hostname, MachineSource::Hostname)
}

/// Trim `value` and return it only if it still holds non-whitespace content.
fn non_empty(value: Option<String>) -> Option<String> {
    value
        .map(|raw| raw.trim().to_string())
        .filter(|trimmed| !trimmed.is_empty())
}

/// The system hostname as a lossy UTF-8 string.
fn hostname() -> String {
    gethostname::gethostname().to_string_lossy().into_owned()
}

/// Read the `name` field from `machine.local.toml`, or `None` when the file is absent, unparsable,
/// or has no `name` set.
fn read_machine_file() -> Option<String> {
    let text = std::fs::read_to_string(machine_config_path()).ok()?;
    toml::from_str::<MachineToml>(&text).ok()?.name
}

/// Persist `name` as this machine's identity to `machine.local.toml`, creating the config dir if
/// needed. The name is trimmed; an empty name is rejected.
pub fn set_machine(name: &str) -> std::io::Result<()> {
    let name = name.trim();
    if name.is_empty() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "machine name must not be empty",
        ));
    }
    let path = machine_config_path();
    // The machine-config path is always `<config dir>/machine.local.toml`, so it always has a parent.
    crate::utils::fs_perms::create_private_dir_all(
        path.parent().expect("machine config path has a parent dir"),
    )?;
    let toml = MachineToml {
        name: Some(name.to_string()),
    };
    let text = toml::to_string_pretty(&toml)
        .expect("MachineToml serialization cannot fail for a struct with an Option<String> field");
    atomic_write(&path, text.as_bytes())
}

/// Distinct machine names referenced across all on-disk routines.
///
/// There is no central registry of machines, so the "known" set is the union of every `machines`
/// targeting list the config repo declares. Backs `moadim machine list`. Reads straight from disk so
/// it works without a running daemon.
pub fn referenced_machines() -> std::collections::BTreeSet<String> {
    let mut names = std::collections::BTreeSet::new();
    let routines = crate::routine_storage::load_store();
    for routine in routines.lock_recover().values() {
        names.extend(routine.machines.iter().cloned());
    }
    names
}

/// `true` if an entry targeting `machines` should run on the machine named `me`.
///
/// An empty list targets *no* machine (dormant until assigned), so an entry runs only when its list
/// explicitly names this machine. Used by the routine crontab sync filter.
pub fn targets(machines: &[String], me: &str) -> bool {
    machines.iter().any(|name| name == me)
}

/// Reject `machines` entries that are empty or whitespace-only, and return a normalized copy with
/// each entry trimmed and duplicates collapsed (first occurrence wins, order preserved).
///
/// Matching in [`targets`] is exact and case-sensitive (`name == me`), so an untrimmed or empty
/// entry can never match the resolved machine name — yet a non-empty list slips past the
/// dormant-routine warning, producing an entry that runs nowhere with no warning at all. Validating
/// here is the authoritative server-side seam shared by the routine and cron-job create/update
/// paths (mirroring `validate_repositories`/`validate_title`), so the REST API, the MCP tools, and a
/// hand-edited `routine.toml`/`job.toml` all reject identically (#600).
pub fn validate_machines(machines: &[String]) -> Result<Vec<String>, AppError> {
    let mut normalized: Vec<String> = Vec::with_capacity(machines.len());
    for (index, name) in machines.iter().enumerate() {
        let trimmed = name.trim();
        if trimmed.is_empty() {
            return Err(AppError::BadRequest(format!(
                "machines[{index}] must not be empty or whitespace-only"
            )));
        }
        if !normalized.iter().any(|existing| existing == trimmed) {
            normalized.push(trimmed.to_string());
        }
    }
    Ok(normalized)
}

/// Run the `moadim machine` CLI subcommand, returning the process exit code.
pub fn run(args: &[String]) -> i32 {
    match args.first().map(String::as_str) {
        None | Some("show") => cmd_show(),
        Some("set") => match args.get(1) {
            Some(name) => cmd_set(name),
            None => {
                eprintln!("usage: moadim machine set <name>");
                2
            }
        },
        Some("list") => cmd_list(),
        Some(other) => {
            eprintln!("unknown machine subcommand {other:?}; expected show, set, or list");
            2
        }
    }
}

/// `moadim machine show` — print the resolved machine name and where it came from.
fn cmd_show() -> i32 {
    let (name, source) = resolve();
    println!("{name} (from {})", source.label());
    0
}

/// `moadim machine set <name>` — persist the machine identity.
fn cmd_set(name: &str) -> i32 {
    match set_machine(name) {
        Ok(()) => {
            println!("machine name set to {:?}", name.trim());
            0
        }
        Err(err) => {
            eprintln!("error: failed to set machine name: {err}");
            1
        }
    }
}

/// `moadim machine list` — print the distinct machine names referenced by routines/jobs.
fn cmd_list() -> i32 {
    let names = referenced_machines();
    if names.is_empty() {
        println!("no machines referenced by any routine");
    } else {
        for name in &names {
            println!("{name}");
        }
    }
    0
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod machine_tests;
