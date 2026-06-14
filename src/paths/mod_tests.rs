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
