#![allow(clippy::missing_docs_in_private_items)]

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
fn routine_toml_path_filename() {
    let path = routine_toml_path("abc");
    assert_eq!(path.file_name().unwrap().to_str().unwrap(), "routine.toml");
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
        "prompt.compiled.md"
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
fn pid_file_ends_with_moadim_pid() {
    let path = pid_file();
    assert!(
        path.to_string_lossy().ends_with("moadim.pid"),
        "expected path to end with 'moadim.pid': {}",
        path.display()
    );
    assert_eq!(path.parent().unwrap(), config_dir());
}

#[test]
fn daemon_log_file_ends_with_daemon_log() {
    let path = daemon_log_file();
    assert!(
        path.to_string_lossy().ends_with("daemon.log"),
        "expected path to end with 'daemon.log': {}",
        path.display()
    );
    assert_eq!(path.parent().unwrap(), config_dir());
}

#[test]
fn user_prompt_path_filename() {
    let path = user_prompt_path();
    assert_eq!(
        path.file_name().unwrap().to_str().unwrap(),
        "user_prompt.md"
    );
    assert!(path.to_string_lossy().contains("moadim"));
}

#[test]
fn user_prompt_path_is_in_config_dir() {
    assert_eq!(user_prompt_path().parent().unwrap(), config_dir());
}

#[test]
fn config_root_from_absolute_xdg_is_used_verbatim() {
    // An absolute `$XDG_CONFIG_HOME` relocates the config root, ignoring `home`.
    let xdg = std::ffi::OsString::from("/custom/xdg");
    let home = Some(std::path::PathBuf::from("/home/u"));
    let dir = super::config_root_from(Some(xdg), home);
    assert_eq!(dir, std::path::PathBuf::from("/custom/xdg"));
}

#[test]
fn config_root_from_relative_xdg_is_ignored() {
    // A relative `$XDG_CONFIG_HOME` violates the XDG spec and must be ignored, falling back to
    // `$HOME/.config`.
    let xdg = std::ffi::OsString::from("relative/config");
    let home = Some(std::path::PathBuf::from("/home/u"));
    let dir = super::config_root_from(Some(xdg), home);
    assert_eq!(dir, std::path::PathBuf::from("/home/u/.config"));
}

#[test]
fn config_root_from_unset_xdg_falls_back_to_home_config() {
    let home = Some(std::path::PathBuf::from("/home/u"));
    let dir = super::config_root_from(None, home);
    assert_eq!(dir, std::path::PathBuf::from("/home/u/.config"));
}

#[test]
fn config_root_from_none_home_falls_back_to_dot() {
    // Exercises the `home.unwrap_or_else(|| PathBuf::from("."))` fallback for the case where both
    // `$XDG_CONFIG_HOME` is unset and `dirs::home_dir()` yields `None`.
    let dir = super::config_root_from(None, None);
    assert!(dir.ends_with(".config"));
    assert!(dir.starts_with("."));
}

#[test]
fn config_dir_nests_moadim_under_config_root() {
    assert!(config_dir().ends_with("moadim"));
    assert_eq!(config_dir().file_name().unwrap(), "moadim");
}
