/// Resolve the default home directory, using an isolated per-test root in test builds.
#[cfg(not(test))]
pub(crate) fn default_home() -> Option<std::path::PathBuf> {
    dirs::home_dir()
}

/// Resolve an isolated home directory for this test process when no fixture override is installed.
///
/// One root per process, not per thread: a test that spawns helper threads must see the same
/// home on all of them, or setup written on the test thread is invisible to the work it checks.
#[cfg(test)]
pub(crate) fn default_home() -> Option<std::path::PathBuf> {
    Some(std::env::temp_dir().join(format!("moadim-test-home-{}", std::process::id())))
}
