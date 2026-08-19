
/// Write `toml` to `machine.local.toml`, creating the config dir if needed.
fn write_machine_toml(toml: &MachineToml) -> std::io::Result<()> {
    let path = machine_config_path();
    // The machine-config path is always `<config dir>/machine.local.toml`, so it always has a parent.
    let parent = crate::utils::fs_perms::parent_or_err(&path, "machine config")?;
    crate::utils::fs_perms::create_private_dir_all(parent)?;
    let text = toml::to_string_pretty(toml).map_err(std::io::Error::other)?;
    atomic_write(&path, text.as_bytes())
}

/// Persist `name` as this machine's identity to `machine.local.toml`, creating the config dir if
/// needed. The name is trimmed; an empty name is rejected. Preserves any other field already
/// persisted in the file (e.g. the concurrency-cap override, see [`max_concurrent_runs_override`]).
pub fn set_machine(name: &str) -> std::io::Result<()> {
    let name = name.trim();
    if name.is_empty() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "machine name must not be empty",
        ));
    }
    let _guard = machine_toml_lock().lock_recover();
    let mut toml = read_machine_toml();
    toml.name = Some(name.to_string());
    write_machine_toml(&toml)
}

/// UI/REST-configured override for the global routine concurrency cap, read from
/// `machine.local.toml`. `None` when unset or the file is absent/unparsable — callers fall back to
/// the `MOADIM_MAX_CONCURRENT_RUNS` env var, then the built-in default.
pub fn max_concurrent_runs_override() -> Option<usize> {
    read_machine_toml().max_concurrent_runs
}

/// Persist a UI/REST-configured override for the global routine concurrency cap to
/// `machine.local.toml`. `None` clears the override. Preserves any other field already persisted
/// in the file (e.g. the machine name).
pub fn set_max_concurrent_runs_override(value: Option<usize>) -> std::io::Result<()> {
    let _guard = machine_toml_lock().lock_recover();
    let mut toml = read_machine_toml();
    toml.max_concurrent_runs = value;
    write_machine_toml(&toml)
}

/// Distinct machine names referenced across all on-disk routines.
///
/// There is no central registry of machines, so the "known" set is the union of every `machines`
/// targeting list the config repo declares. Backs `moadim machine list`. Reads straight from disk so
/// it works without a running daemon.
pub fn referenced_machines() -> std::collections::BTreeSet<String> {
    let mut names = std::collections::BTreeSet::new();
    let routines = crate::routine_storage::load_store();
    #[allow(
        clippy::iter_over_hash_type,
        reason = "every name lands in the sorted BTreeSet, so hash-map iteration order is irrelevant"
    )]
    for routine in routines.lock_recover().values() {
        names.extend(routine.machines.iter().cloned());
    }
    names
}

/// `true` if an entry targeting `machines` should run on the machine named `me`.
///
/// An empty list targets *no* machine (dormant until assigned), so an entry runs only when its list
/// explicitly names this machine. Each entry matches either by exact string equality, or — if it
/// contains `*` — as a glob where `*` stands for any (possibly empty) run of characters (e.g. `"*"`
/// matches every machine, `"box-*"` matches `"box-1"` and `"box-2"`). Used by the routine crontab
/// sync filter. See issue #1393.
pub fn targets(machines: &[String], me: &str) -> bool {
    machines.iter().any(|pattern| {
        if pattern.contains('*') {
            glob_match(pattern, me)
        } else {
            pattern == me
        }
    })
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod machine_tests;

#[cfg(test)]
#[path = "mod_concurrency_tests.rs"]
mod machine_concurrency_tests;
