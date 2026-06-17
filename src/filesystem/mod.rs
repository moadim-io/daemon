/// Server filesystem location, injected into response headers.
#[derive(serde::Serialize)]
pub struct FsLocation {
    /// Absolute path of the server's working directory.
    pub server_root: Option<String>,
    /// Absolute path of the directory containing the server executable.
    pub server_exe_dir: Option<String>,
}

impl FsLocation {
    /// Capture the current working directory and executable directory.
    pub fn current() -> Self {
        Self {
            server_root: std::env::current_dir()
                .ok()
                .map(|path| path.to_string_lossy().into_owned()),
            server_exe_dir: std::env::current_exe()
                .ok()
                .and_then(|path| path.parent().map(|dir| dir.to_string_lossy().into_owned())),
        }
    }
}

#[cfg(test)]
#[path = "fs_location_tests.rs"]
mod fs_location_tests;
