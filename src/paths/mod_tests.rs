#![allow(
    clippy::missing_docs_in_private_items,
    reason = "test helpers and fixtures do not need doc comments"
)]

use super::*;

#[test]

fn machine_config_path_filename() {
    let path = machine_config_path();
    assert_eq!(
        path.file_name().unwrap().to_str().unwrap(),
        "machine.local.toml"
    );
    assert!(path.to_string_lossy().contains("moadim"));
}

#[test]
fn machine_config_path_from_home_none_falls_back_to_dot() {
    let dir = super::machine_config_path_from_home(None);
    assert!(dir.ends_with(".config/moadim/machine.local.toml"));
    assert!(dir.starts_with("."));
}

#[test]
fn routines_dir_ends_with_routines() {
    let path = routines_dir().to_string_lossy().into_owned();
    assert!(path.contains("moadim"), "expected 'moadim' in {path}");
    assert!(
        path.ends_with("routines"),
        "expected end with 'routines': {path}"
    );
}

#[test]
fn routine_dir_is_child_of_routines_dir() {
    assert_eq!(routine_dir("xyz").parent().unwrap(), routines_dir());
}

#[test]
fn routines_readme_path_in_routines_dir() {
    let path = routines_readme_path();
    assert_eq!(path.file_name().unwrap().to_str().unwrap(), "README.md");
    assert_eq!(path.parent().unwrap(), routines_dir());
}

#[test]
fn routine_toml_path_filename() {
    let path = routine_toml_path("abc");
    assert_eq!(path.file_name().unwrap().to_str().unwrap(), "routine.toml");
    assert!(path.to_string_lossy().contains("abc"));
}

#[test]
fn routine_cron_path_filename() {
    let path = routine_cron_path("abc");
    assert_eq!(path.file_name().unwrap().to_str().unwrap(), "schedule.cron");
    assert!(path.to_string_lossy().contains("abc"));
}

#[test]
fn routine_prompts_dir_filename() {
    let path = routine_prompts_dir("abc");
    assert_eq!(path.file_name().unwrap().to_str().unwrap(), "prompts");
    assert_eq!(path.parent().unwrap(), routine_dir("abc"));
}

#[test]
fn routine_pure_prompt_path_filename() {
    let path = routine_pure_prompt_path("abc");
    assert_eq!(
        path.file_name().unwrap().to_str().unwrap(),
        "prompt.pure.md"
    );
    assert_eq!(path.parent().unwrap(), routine_prompts_dir("abc"));
}

#[test]
fn routine_compiled_prompt_path_filename() {
    let path = routine_compiled_prompt_path("abc");
    assert_eq!(
        path.file_name().unwrap().to_str().unwrap(),
        "prompt.compiled.local.md"
    );
    assert_eq!(path.parent().unwrap(), routine_prompts_dir("abc"));
}

#[test]
fn routine_gitignore_path_filename() {
    let path = routine_gitignore_path("abc");
    assert_eq!(path.file_name().unwrap().to_str().unwrap(), ".gitignore");
    assert!(path.to_string_lossy().contains("abc"));
}

#[test]
fn routine_state_path_filename() {
    let path = routine_state_path("abc");
    assert_eq!(
        path.file_name().unwrap().to_str().unwrap(),
        "state.local.toml"
    );
    assert_eq!(path.parent().unwrap(), routine_dir("abc"));
}

#[test]
fn routine_flags_dir_is_child_of_routine_dir() {
    let path = routine_flags_dir("abc");
    assert_eq!(path.file_name().unwrap().to_str().unwrap(), "flags");
    assert_eq!(path.parent().unwrap(), routine_dir("abc"));
}

#[test]
fn agents_dir_ends_with_agents() {
    let path = agents_dir().to_string_lossy().into_owned();
    assert!(
        path.ends_with("agents"),
        "expected end with 'agents': {path}"
    );
}

#[test]
fn agent_toml_path_appends_name_and_extension() {
    let path = agent_toml_path("claude");
    assert_eq!(path.file_name().unwrap().to_str().unwrap(), "claude.toml");
}

#[test]
fn agents_readme_path_in_agents_dir() {
    let path = agents_readme_path();
    assert_eq!(path.file_name().unwrap().to_str().unwrap(), "README.md");
    assert_eq!(path.parent().unwrap(), agents_dir());
}

#[test]
fn moadim_home_ends_with_dot_moadim() {
    assert!(moadim_home().ends_with(".moadim"));
}

#[test]
fn moadim_home_from_home_none_falls_back_to_dot() {
    let dir = super::moadim_home_from_home(None);
    assert!(dir.ends_with(".moadim"));
    assert!(dir.starts_with("."));
}

#[test]
fn workbenches_dir_under_moadim_home() {
    let path = workbenches_dir();
    assert!(path.ends_with("workbenches"));
    assert_eq!(path.parent().unwrap(), moadim_home());
}

#[test]
fn config_gitignore_path_in_config_dir() {
    let path = config_gitignore_path();
    assert_eq!(path.file_name().unwrap().to_str().unwrap(), ".gitignore");
    assert_eq!(path.parent().unwrap(), config_dir());
}

#[test]
fn config_readme_path_in_config_dir() {
    let path = config_readme_path();
    assert_eq!(path.file_name().unwrap().to_str().unwrap(), "README.md");
    assert_eq!(path.parent().unwrap(), config_dir());
}
include!("mod_tests_part2.rs");
