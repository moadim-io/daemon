#[derive(serde::Serialize)]
pub struct FsLocation {
    pub server_root: Option<String>,
    pub server_exe_dir: Option<String>,
}

impl FsLocation {
    pub fn current() -> Self {
        Self {
            server_root: std::env::current_dir()
                .ok()
                .map(|p| p.to_string_lossy().into_owned()),
            server_exe_dir: std::env::current_exe()
                .ok()
                .and_then(|p| p.parent().map(|d| d.to_string_lossy().into_owned())),
        }
    }
}
