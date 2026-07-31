
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
fn install_prompt_marker_path_filename() {
    let path = install_prompt_marker_path();
    assert_eq!(
        path.file_name().unwrap().to_str().unwrap(),
        "install_prompt.local.marker"
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

#[test]
fn claude_json_path_is_dot_claude_json_under_home() {
    let previous = std::env::var_os("MOADIM_HOME_OVERRIDE");
    // SAFETY: tests in this crate run single-threaded (RUST_TEST_THREADS=1); restored below.
    unsafe {
        std::env::set_var("MOADIM_HOME_OVERRIDE", "/home/u");
    }

    let path = claude_json_path();

    // SAFETY: single-threaded harness; restore the saved override.
    unsafe {
        match previous {
            Some(value) => std::env::set_var("MOADIM_HOME_OVERRIDE", value),
            None => std::env::remove_var("MOADIM_HOME_OVERRIDE"),
        }
    }

    assert_eq!(path, Some(std::path::PathBuf::from("/home/u/.claude.json")));
}

#[test]
fn sanitize_repo_cache_name_empty_url_falls_back_to_repo() {
    assert_eq!(super::sanitize_repo_cache_name(""), "repo");
}

#[test]
fn repo_cache_root_dir_ends_with_cache_under_config_dir() {
    let path = repo_cache_root_dir();
    assert_eq!(path.file_name().unwrap().to_str().unwrap(), "cache");
    assert_eq!(path.parent().unwrap(), config_dir());
}

#[test]
fn repo_cache_dir_is_child_of_repo_cache_root_dir() {
    let path = repo_cache_dir("https://example.com/a/b.git");
    assert_eq!(path.parent().unwrap(), repo_cache_root_dir());
}
