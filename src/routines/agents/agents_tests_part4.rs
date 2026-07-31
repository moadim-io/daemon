
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
