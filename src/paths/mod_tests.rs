#![allow(clippy::missing_docs_in_private_items)]

use super::*;

#[test]
fn jobs_dir_contains_moadim_and_ends_with_jobs() {
    let p = jobs_dir().to_string_lossy().into_owned();
    assert!(p.contains("moadim"), "expected 'moadim' in {p}");
    assert!(p.ends_with("jobs"), "expected path to end with 'jobs': {p}");
}

#[test]
fn job_dir_appends_id() {
    let p = job_dir("my-id").to_string_lossy().into_owned();
    assert!(
        p.ends_with("my-id"),
        "expected path to end with 'my-id': {p}"
    );
}

#[test]
fn job_toml_path_filename() {
    let p = job_toml_path("abc");
    assert_eq!(p.file_name().unwrap().to_str().unwrap(), "job.toml");
    assert!(p.to_string_lossy().contains("abc"));
}

#[test]
fn job_local_toml_path_filename() {
    let p = job_local_toml_path("abc");
    assert_eq!(p.file_name().unwrap().to_str().unwrap(), "job.local.toml");
    assert!(p.to_string_lossy().contains("abc"));
}

#[test]
fn job_gitignore_path_filename() {
    let p = job_gitignore_path("abc");
    assert_eq!(p.file_name().unwrap().to_str().unwrap(), ".gitignore");
    assert!(p.to_string_lossy().contains("abc"));
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
    let p = handlers_dir().to_string_lossy().into_owned();
    assert!(p.contains("moadim"), "expected 'moadim' in {p}");
    assert!(
        p.ends_with("handlers"),
        "expected path to end with 'handlers': {p}"
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
    let p = super::job_log_path("abc");
    assert_eq!(p.file_name().unwrap().to_str().unwrap(), "job.local.log");
    assert!(p.to_string_lossy().contains("abc"));
}

#[test]
fn routines_dir_ends_with_routines() {
    let p = routines_dir().to_string_lossy().into_owned();
    assert!(p.contains("moadim"), "expected 'moadim' in {p}");
    assert!(p.ends_with("routines"), "expected end with 'routines': {p}");
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
    let p = routine_toml_path("abc");
    assert_eq!(p.file_name().unwrap().to_str().unwrap(), "routine.toml");
    assert!(p.to_string_lossy().contains("abc"));
}

#[test]
fn routine_prompt_path_filename() {
    let p = routine_prompt_path("abc");
    assert_eq!(p.file_name().unwrap().to_str().unwrap(), "prompt.txt");
    assert!(p.to_string_lossy().contains("abc"));
}

#[test]
fn routine_gitignore_path_filename() {
    let p = routine_gitignore_path("abc");
    assert_eq!(p.file_name().unwrap().to_str().unwrap(), ".gitignore");
    assert!(p.to_string_lossy().contains("abc"));
}

#[test]
fn agents_dir_ends_with_agents() {
    let p = agents_dir().to_string_lossy().into_owned();
    assert!(p.ends_with("agents"), "expected end with 'agents': {p}");
}

#[test]
fn agents_dir_from_home_none_falls_back_to_dot() {
    let dir = super::agents_dir_from_home(None);
    assert!(dir.ends_with(".config/moadim/agents"));
    assert!(dir.starts_with("."));
}

#[test]
fn agent_toml_path_appends_name_and_extension() {
    let p = agent_toml_path("claude");
    assert_eq!(p.file_name().unwrap().to_str().unwrap(), "claude.toml");
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
    let p = workbenches_dir();
    assert!(p.ends_with("workbenches"));
    assert_eq!(p.parent().unwrap(), moadim_home());
}
