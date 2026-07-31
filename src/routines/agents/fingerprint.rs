
/// A non-cryptographic 64-bit fingerprint of `data`, rendered as lowercase hex.
///
/// ponytail: FNV-1a, not a cryptographic hash — fine here since this only tells "pristine" content
/// apart from "edited" content, not a security boundary. Deterministic across Rust versions (unlike
/// `std::collections::hash_map::DefaultHasher`, whose algorithm is not guaranteed stable), which
/// matters because a fingerprint written by one daemon build is compared against one computed by a
/// later build.
fn fingerprint(data: &str) -> String {
    const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const FNV_PRIME: u64 = 0x0100_0000_01b3;
    let mut hash = FNV_OFFSET;
    for byte in data.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    format!("{hash:016x}")
}

/// Render `contents` as a daemon-managed file: [`MANAGED_HEADER_PREFIX`] plus its fingerprint, then
/// `contents` verbatim.
fn render_managed(contents: &str) -> String {
    format!(
        "{MANAGED_HEADER_PREFIX}{}\n{contents}",
        fingerprint(contents)
    )
}

/// Split a daemon-written file's `text` into its recorded fingerprint and body.
///
/// Returns `None` for text with no (well-formed) [`MANAGED_HEADER_PREFIX`] line — a config seeded
/// before this mechanism existed, or never written by the daemon at all — which
/// [`ensure_default_agents_in`] treats as foreign and never touches.
fn parse_managed(text: &str) -> Option<(&str, &str)> {
    let rest = text.strip_prefix(MANAGED_HEADER_PREFIX)?;
    rest.split_once('\n')
}

/// Write any missing built-in agent configs into `~/.config/moadim/agents/`, and refresh any
/// existing one that is still pristine (unedited since the daemon wrote it) but stale.
///
/// A user-modified config is never overwritten — only a file whose current content still matches the
/// fingerprint recorded when the daemon wrote it is refreshed. Best-effort: directory, read, or write
/// failures are logged and ignored rather than aborting startup.
pub fn ensure_default_agents() {
    ensure_default_agents_in(&agents_dir());
}

/// Write missing / refresh stale-pristine built-in agent configs into `dir`. See
/// [`ensure_default_agents`].
pub(crate) fn ensure_default_agents_in(dir: &Path) {
    if let Err(err) = crate::utils::fs_perms::create_private_dir_all(dir) {
        log::warn!(
            "ensure_default_agents: failed to create {}: {err}",
            dir.display()
        );
        return;
    }
    for (name, contents) in DEFAULT_AGENT_CONFIGS {
        let path = dir.join(format!("{name}.toml"));
        match std::fs::read_to_string(&path) {
            Err(err) if err.kind() == ErrorKind::NotFound => {
                if let Err(err) = std::fs::write(&path, render_managed(contents)) {
                    log::warn!(
                        "ensure_default_agents: failed to write {}: {err}",
                        path.display()
                    );
                }
            }
            Err(err) => {
                log::warn!(
                    "ensure_default_agents: failed to read {}: {err}",
                    path.display()
                );
            }
            Ok(existing) => {
                let Some((recorded_fingerprint, body)) = parse_managed(&existing) else {
                    // No managed header: a foreign file, or one seeded before this mechanism
                    // existed. Its provenance is unknown, so it's left strictly alone.
                    continue;
                };
                if body == *contents {
                    continue; // Already current; nothing to do.
                }
                if fingerprint(body) != recorded_fingerprint {
                    // Edited since the daemon last wrote it: never overwrite a user's edit.
                    continue;
                }
                // A pristine copy of a stale default: safe to refresh to the current built-in.
                if let Err(err) = std::fs::write(&path, render_managed(contents)) {
                    log::warn!(
                        "ensure_default_agents: failed to rewrite stale default {}: {err}",
                        path.display()
                    );
                } else {
                    log::info!(
                        "ensure_default_agents: upgraded stale default {} to the current built-in",
                        path.display()
                    );
                }
            }
        }
    }
}

#[cfg(test)]
#[path = "agents_tests.rs"]
mod agents_tests;
