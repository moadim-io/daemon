/// Resolve the default home directory, using an isolated per-test root in test builds.
#[cfg(not(test))]
pub(crate) fn default_home() -> Option<std::path::PathBuf> {
    dirs::home_dir()
}

/// Resolve an isolated per-test home directory when no fixture override is installed.
#[cfg(test)]
pub(crate) fn default_home() -> Option<std::path::PathBuf> {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    std::thread::current()
        .name()
        .unwrap_or("unnamed-test-thread")
        .hash(&mut hasher);
    Some(
        std::env::temp_dir()
            .join(format!("moadim-test-home-{}", std::process::id()))
            .join(format!("{:016x}", hasher.finish())),
    )
}
