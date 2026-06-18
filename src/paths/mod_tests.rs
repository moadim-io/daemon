#![allow(clippy::missing_docs_in_private_items)]

use super::*;

#[test]
fn jobs_dir_contains_moadim_and_ends_with_jobs() {
    let path = jobs_dir().to_string_lossy().into_owned();
    assert!(path.contains("moadim"), "expected 'moadim' in {path}");
    assert!(
        path.ends_with("jobs"),
        "expected path to end with 'jobs': {path}"
    );
}

#[test]
fn job_dir_appends_id() {
    let path = job_dir("my-id").to_string_lossy().into_owned();
    assert!(
        path.ends_with("my-id"),
        "expected path to end with 'my-id': {path}"
    );
}

#[test]
fn job_toml_path_filename() {
    let path = job_toml_path("abc");
    assert_eq!(path.file_name().unwrap().to_str().unwrap(), "job.toml");
    assert!(path.to_string_lossy().contains("abc"));
}

#[test]
fn job_local_toml_path_filename() {
    let path = job_local_toml_path("abc");
    assert_eq!(
        path.file_name().unwrap().to_str().unwrap(),
        "job.local.toml"
    );
    assert!(path.to_string_lossy().contains("abc"));
}

#[test]
fn job_gitignore_path_filename() {
    let path = job_gitignore_path("abc");
    assert_eq!(path.file_name().unwrap().to_str().unwrap(), ".gitignore");
    assert!(path.to_string_lossy().contains("abc"));
}

#[test]
fn job_dir_is_child_of_jobs_dir() {
    let base = jobs_dir();
    let child = job_dir("xyz");
    assert_eq!(child.parent().unwrap(), base);
}

#[test]
fn jobs_dir_from_home_none_falls_back_to_dot() {
    let dir = super::jobs_dir_from_home(None);
    assert!(dir.ends_with(".config/moadim/jobs"));
    assert!(dir.starts_with("."));
}

#[test]
fn handlers_dir_contains_moadim_and_ends_with_handlers() {
    let path = handlers_dir().to_string_lossy().into_owned();
    assert!(path.contains("moadim"), "expected 'moadim' in {path}");
    assert!(
        path.ends_with("handlers"),
        "expected path to end with 'handlers': {path}"
    );
}

#[test]
fn handlers_dir_from_home_none_falls_back_to_dot() {
    let dir = super::handlers_dir_from_home(None);
    assert!(dir.ends_with(".config/moadim/handlers"));
    assert!(dir.starts_with("."));
}

#[test]
fn job_log_path_filename() {
    let path = super::job_log_path("abc");
    assert_eq!(path.file_name().unwrap().to_str().unwrap(), "job.local.log");
    assert!(path.to_string_lossy().contains("abc"));
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
fn routines_dir_from_home_none_falls_back_to_dot() {
    let dir = super::routines_dir_from_home(None);
    assert!(dir.ends_with(".config/moadim/routines"));
    assert!(dir.starts_with("."));
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
fn routine_prompt_path_filename() {
    let path = routine_prompt_path("abc");
    assert_eq!(path.file_name().unwrap().to_str().unwrap(), "prompt.md");
    assert!(path.to_string_lossy().contains("abc"));
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
fn agents_dir_ends_with_agents() {
    let path = agents_dir().to_string_lossy().into_owned();
    assert!(
        path.ends_with("agents"),
        "expected end with 'agents': {path}"
    );
}

#[test]
fn agents_dir_from_home_none_falls_back_to_dot() {
    let dir = super::agents_dir_from_home(None);
    assert!(dir.ends_with(".config/moadim/agents"));
    assert!(dir.starts_with("."));
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
fn user_prompt_path_from_home_none_falls_back_to_dot() {
    let path = super::user_prompt_path_from_home(None);
    assert!(path.ends_with(".config/moadim/user_prompt.md"));
    assert!(path.starts_with("."));
}

#[test]
fn user_prompt_path_is_in_config_dir() {
    assert_eq!(user_prompt_path().parent().unwrap(), config_dir());
}

#[test]
fn config_dir_from_home_none_falls_back_to_dot() {
    // Exercises the `home.unwrap_or_else(|| PathBuf::from("."))` fallback in
    // `config_dir_from_home` for the case where `dirs::home_dir()` yields `None`.
    let dir = super::config_dir_from_home(None);
    assert!(dir.ends_with(".config/moadim"));
    assert!(dir.starts_with("."));
}

#[test]
fn config_dir_from_home_some_joins_under_home() {
    // The `Some(home)` arm: the moadim config dir nests under the provided home.
    let home = std::path::PathBuf::from("/tmp/some-home");
    let dir = super::config_dir_from_home(Some(home.clone()));
    assert_eq!(dir, home.join(".config").join("moadim"));
}
