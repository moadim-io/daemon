#![allow(
    clippy::missing_docs_in_private_items,
    reason = "test helpers and fixtures do not need doc comments"
)]

use super::*;
use crate::routines::{slugify, Repository, Routine};

fn scratch_dir(tag: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("moadim-rs-{tag}-{}", uuid::Uuid::new_v4()))
}

fn with_override_home(body: impl FnOnce(&std::path::Path)) {
    let home = scratch_dir("override-home-env");
    std::fs::create_dir_all(&home).unwrap();
    let previous = std::env::var_os("MOADIM_HOME_OVERRIDE");
    // SAFETY: tests in this crate run single-threaded per binary.
    unsafe {
        std::env::set_var("MOADIM_HOME_OVERRIDE", &home);
    }
    body(&home);
    // SAFETY: single-threaded test execution.
    unsafe {
        match previous {
            Some(value) => std::env::set_var("MOADIM_HOME_OVERRIDE", value),
            None => std::env::remove_var("MOADIM_HOME_OVERRIDE"),
        }
    }
    let _ = std::fs::remove_dir_all(&home);
}

fn make_routine(id: &str, title: &str) -> Routine {
    Routine {
        model: None,
        id: id.to_string(),
        schedule: "@daily".to_string(),
        title: title.to_string(),
        agent: "claude".to_string(),
        prompt: "task".to_string(),
        goal: None,
        repositories: vec![Repository {
            repository: "https://example.com/r.git".to_string(),
            branch: Some("main".to_string()),
        }],
        machines: vec![crate::machine::current_machine()],
        enabled: true,
        source: "managed".to_string(),
        created_at: 5,
        updated_at: 6,
        last_manual_trigger_at: None,
        last_scheduled_trigger_at: None,
        snoozed_until: None,
        skip_runs: None,
        power_saving: false,
        tags: vec![],
        ttl_secs: None,
        max_runtime_secs: None,
        env: std::collections::HashMap::new(),
    }
}

#[test]
fn write_routine_persists_env_table_and_load_reads_it_back() {
    // `routine.toml`'s `[env]` table round-trips through `write_routine` / `load_store`
    // (#408): absent/empty behaves as before, a non-empty map is written and reloaded intact.
    with_override_home(|_home| {
        let id = "rs-env-roundtrip-id";
        let title = "Rs Env Roundtrip Routine";
        let mut routine = make_routine(id, title);
        routine.env = std::collections::HashMap::from([
            ("MODEL_OVERRIDE".to_string(), "gpt-x".to_string()),
            ("BASE_URL".to_string(), "https://example.test".to_string()),
        ]);
        write_routine(&routine).unwrap();

        let slug = slugify(title);
        let toml_text = std::fs::read_to_string(crate::paths::routine_toml_path(&slug)).unwrap();
        assert!(
            toml_text.contains("[env]"),
            "expected an [env] table in routine.toml, got:\n{toml_text}"
        );

        let store = load_store();
        let loaded = store.lock().unwrap().get(id).cloned().unwrap();
        assert_eq!(
            loaded.env.get("MODEL_OVERRIDE").map(String::as_str),
            Some("gpt-x")
        );
        assert_eq!(
            loaded.env.get("BASE_URL").map(String::as_str),
            Some("https://example.test")
        );
    });
}

#[test]
fn write_routine_omits_env_table_when_empty() {
    // An empty `env` map must not spuriously add a `[env]` table — keeps `routine.toml` diffs
    // clean for routines that never opted into the feature.
    with_override_home(|_home| {
        let id = "rs-env-empty-id";
        let title = "Rs Env Empty Routine";
        write_routine(&make_routine(id, title)).unwrap();
        let slug = slugify(title);
        let toml_text = std::fs::read_to_string(crate::paths::routine_toml_path(&slug)).unwrap();
        assert!(
            !toml_text.contains("[env]"),
            "did not expect an [env] table with no env vars set, got:\n{toml_text}"
        );
    });
}

#[test]
fn routine_local_toml_path_matches_the_gitignored_local_pattern() {
    // `routine.local.toml`'s `.local.` infix must match the `*.local.*` pattern the config
    // `.gitignore` already seeds (`cli_system::ensure_config_gitignore`), so it never needs its
    // own bespoke pattern — mirrors `state.local.toml` / `prompt.compiled.local.md` (#408).
    let path = crate::paths::routine_local_toml_path("some-slug");
    let name = path.file_name().unwrap().to_str().unwrap();
    assert_eq!(name, "routine.local.toml");
    assert!(
        name.starts_with("routine.") && name.contains(".local."),
        "expected the *.local.* gitignore glob to match {name:?}"
    );
}

#[test]
fn read_local_env_returns_empty_map_when_sidecar_absent() {
    with_override_home(|_home| {
        assert!(read_local_env("no-such-slug").is_empty());
        assert!(local_env_keys("no-such-slug").is_empty());
    });
}

#[test]
fn read_local_env_reads_the_sidecar_and_local_env_keys_drops_values() {
    with_override_home(|_home| {
        let slug = "rs-local-env-read-slug";
        let dir = crate::paths::routine_dir(slug);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            crate::paths::routine_local_toml_path(slug),
            "[env]\nGH_TOKEN = \"ghp_super_secret\"\n",
        )
        .unwrap();

        let values = read_local_env(slug);
        assert_eq!(
            values.get("GH_TOKEN").map(String::as_str),
            Some("ghp_super_secret")
        );

        let keys = local_env_keys(slug);
        assert_eq!(keys, vec!["GH_TOKEN".to_string()]);
    });
}
