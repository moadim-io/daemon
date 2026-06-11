//! Path builders for the moadim jobs directory layout.

use std::path::PathBuf;

/// Returns the path to `~/.config/moadim/jobs/`.
pub fn jobs_dir() -> PathBuf {
    jobs_dir_from_home(dirs::home_dir())
}

/// Returns the jobs directory under `home`, or `.` if `home` is `None`.
pub(crate) fn jobs_dir_from_home(home: Option<PathBuf>) -> PathBuf {
    home.unwrap_or_else(|| PathBuf::from("."))
        .join(".config")
        .join("moadim")
        .join("jobs")
}

/// Returns the path to `{jobs_dir}/{id}/`.
pub fn job_dir(id: &str) -> PathBuf {
    jobs_dir().join(id)
}

/// Returns the path to `{jobs_dir}/{id}/job.toml`.
pub fn job_toml_path(id: &str) -> PathBuf {
    job_dir(id).join("job.toml")
}

/// Returns the path to `{jobs_dir}/{id}/job.local.toml`.
pub fn job_local_toml_path(id: &str) -> PathBuf {
    job_dir(id).join("job.local.toml")
}

/// Returns the path to `{jobs_dir}/{id}/.gitignore`.
pub fn job_gitignore_path(id: &str) -> PathBuf {
    job_dir(id).join(".gitignore")
}

#[cfg(test)]
mod mod_tests;
