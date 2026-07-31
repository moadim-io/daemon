#![allow(
    clippy::wildcard_imports,
    clippy::too_many_arguments,
    clippy::needless_pass_by_value,
    dead_code,
    reason = "split-out module preserves existing code while keeping files under the linecheck limit"
)]
use super::*;

#[cfg(unix)]
#[test]
fn ensure_default_agents_in_swallows_per_config_write_errors() {
    use std::os::unix::fs::PermissionsExt as _;

    // Covers the per-config `std::fs::write` error arm: the directory exists (so `create_dir_all`
    // succeeds) but is read-only, so each config write fails and is logged rather than panicking.
    let dir = unique_dir("write-fail");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o555)).unwrap();

    ensure_default_agents_in(&dir);

    // Restore permissions so cleanup can proceed. (Root bypasses the read-only bit, in which case
    // the writes succeed; the call is exercised either way.)
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755)).unwrap();
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn ensure_default_agents_in_seeds_with_a_managed_fingerprint_header() {
    // Absent -> seeded case (unchanged from before), now also asserting the seeded file carries
    // the managed header this reconciliation is built on.
    let dir = unique_dir("seed-header");
    std::fs::create_dir_all(&dir).unwrap();

    ensure_default_agents_in(&dir);

    let written = std::fs::read_to_string(dir.join("claude.toml")).unwrap();
    let (hash, body) = parse_managed(&written).expect("seeded file must carry the managed header");
    assert_eq!(body, claude_code::CONFIG);
    assert_eq!(hash, fingerprint(claude_code::CONFIG));

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn ensure_default_agents_in_is_a_noop_when_already_current() {
    // A managed file whose body already equals the current built-in must not be rewritten.
    let dir = unique_dir("already-current");
    std::fs::create_dir_all(&dir).unwrap();
    ensure_default_agents_in(&dir);
    let before = std::fs::read_to_string(dir.join("claude.toml")).unwrap();

    ensure_default_agents_in(&dir);

    let after = std::fs::read_to_string(dir.join("claude.toml")).unwrap();
    assert_eq!(before, after);

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn ensure_default_agents_in_upgrades_a_stale_pristine_config() {
    // (a) stale-pristine -> upgraded: a managed file whose body is an old built-in (never edited,
    // as proven by its body still hashing to its own recorded fingerprint) must be refreshed to the
    // current built-in, with a fingerprint recorded for it in turn.
    let dir = unique_dir("stale-pristine");
    std::fs::create_dir_all(&dir).unwrap();
    let stale_body = "command = \"old-claude\"\n";
    std::fs::write(dir.join("claude.toml"), render_managed(stale_body)).unwrap();

    ensure_default_agents_in(&dir);

    let upgraded = std::fs::read_to_string(dir.join("claude.toml")).unwrap();
    let (hash, body) = parse_managed(&upgraded).expect("still managed after upgrade");
    assert_eq!(body, claude_code::CONFIG);
    assert_eq!(hash, fingerprint(claude_code::CONFIG));

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn ensure_default_agents_in_preserves_a_user_edited_config() {
    // (b) user-edited -> preserved: a managed file whose current body no longer hashes to its
    // recorded fingerprint (the user changed it after the daemon wrote it) must never be touched,
    // even though its body also differs from the current built-in.
    let dir = unique_dir("user-edited");
    std::fs::create_dir_all(&dir).unwrap();
    let managed = render_managed("command = \"old-claude\"\n");
    let edited = format!("{managed}\n# a user note appended after seeding\n");
    std::fs::write(dir.join("claude.toml"), &edited).unwrap();

    ensure_default_agents_in(&dir);

    assert_eq!(
        std::fs::read_to_string(dir.join("claude.toml")).unwrap(),
        edited,
        "a user-edited config must never be overwritten"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn ensure_default_agents_in_leaves_a_legacy_unmanaged_config_untouched() {
    // A config with no managed header at all (seeded before this mechanism existed, or hand-authored)
    // has no provenance to trust, so it must be left strictly alone rather than guessed at.
    let dir = unique_dir("legacy-no-header");
    std::fs::create_dir_all(&dir).unwrap();
    let legacy = "command = \"legacy-claude\"\n";
    std::fs::write(dir.join("claude.toml"), legacy).unwrap();

    ensure_default_agents_in(&dir);

    assert_eq!(
        std::fs::read_to_string(dir.join("claude.toml")).unwrap(),
        legacy
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn ensure_default_agents_in_leaves_a_malformed_managed_header_untouched() {
    // Covers `parse_managed`'s `split_once` returning `None`: the header prefix is present but
    // there's no trailing newline/body, so the file can't be trusted as managed and is left alone.
    let dir = unique_dir("malformed-header");
    std::fs::create_dir_all(&dir).unwrap();
    let malformed = format!("{MANAGED_HEADER_PREFIX}deadbeef");
    std::fs::write(dir.join("claude.toml"), &malformed).unwrap();

    ensure_default_agents_in(&dir);

    assert_eq!(
        std::fs::read_to_string(dir.join("claude.toml")).unwrap(),
        malformed
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[cfg(unix)]
#[test]
fn ensure_default_agents_in_logs_when_rewriting_a_stale_pristine_config_fails() {
    use std::os::unix::fs::PermissionsExt as _;

    // Covers the rewrite-failure arm, distinct from the seed-failure arm covered by
    // `ensure_default_agents_in_swallows_per_config_write_errors`: here the file already exists (a
    // pristine-but-stale managed config) and its own permissions block the overwrite.
    let dir = unique_dir("stale-write-fail");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("claude.toml");
    std::fs::write(&path, render_managed("command = \"old-claude\"\n")).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o444)).unwrap();

    ensure_default_agents_in(&dir);

    // Restore permissions so cleanup can proceed. (Root bypasses the read-only bit, in which case
    // the rewrite succeeds; the call is exercised either way.)
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn available_agents_in_ignores_files_without_extension() {
    // Exercises the `path.extension()?` → None branch in the filter_map closure:
    // a file with no extension (e.g. "readme") has extension() == None, so the `?`
    // propagates None and the entry is skipped.  The .toml files are still returned.
    let dir = unique_dir("no-ext");
    std::fs::create_dir_all(&dir).unwrap();
    // A file without any extension — must be ignored.
    std::fs::write(dir.join("readme"), "not an agent").unwrap();
    // A valid agent config — must be returned.
    std::fs::write(dir.join("my-agent.toml"), "command = \"x\"\n").unwrap();

    let agents = available_agents_in(&dir);
    assert!(
        agents.contains(&"my-agent".to_string()),
        "toml file should be listed"
    );
    assert!(
        !agents.iter().any(|n| n == "readme"),
        "no-extension file must be filtered out"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn available_agents_in_ignores_files_with_non_toml_extension() {
    // Extension is Some but != "toml": the entry is filtered out.
    let dir = unique_dir("non-toml-ext");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("notes.txt"), "ignore").unwrap();
    std::fs::write(dir.join("claude.toml"), "command = \"claude\"\n").unwrap();

    let agents = available_agents_in(&dir);
    assert_eq!(agents, vec!["claude".to_string()]);

    let _ = std::fs::remove_dir_all(&dir);
}
