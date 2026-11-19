//! `tunnel_host_suffixes` persistence tests: the key is read per call and survives other writes.

use super::*;

/// Point `MOADIM_HOME_OVERRIDE` at a temp home for one test, restoring the prior value on drop.
struct HomeGuard {
    previous: Option<std::ffi::OsString>,
}

impl HomeGuard {
    fn set(home: &std::path::Path) -> Self {
        let previous = std::env::var_os("MOADIM_HOME_OVERRIDE");
        // SAFETY: single-threaded test execution.
        unsafe { std::env::set_var("MOADIM_HOME_OVERRIDE", home) }
        Self { previous }
    }
}

impl Drop for HomeGuard {
    fn drop(&mut self) {
        // SAFETY: single-threaded test execution.
        unsafe {
            match self.previous.take() {
                Some(value) => std::env::set_var("MOADIM_HOME_OVERRIDE", value),
                None => std::env::remove_var("MOADIM_HOME_OVERRIDE"),
            }
        }
    }
}

fn temp_home(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("moadim-tunnel-{tag}-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).expect("create temp home");
    dir
}

#[test]
fn tunnel_host_suffixes_absent_is_empty() {
    let home = temp_home("absent");
    let _home = HomeGuard::set(&home);
    assert!(tunnel_host_suffixes().is_empty());
}

#[test]
fn tunnel_host_suffixes_read_from_file_each_call() {
    let home = temp_home("read");
    let _home = HomeGuard::set(&home);
    let config = machine_config_path();
    std::fs::create_dir_all(config.parent().unwrap()).unwrap();
    std::fs::write(&config, "tunnel_host_suffixes = [\"trycloudflare.com\"]\n").unwrap();
    assert_eq!(
        tunnel_host_suffixes(),
        vec!["trycloudflare.com".to_string()]
    );

    std::fs::write(&config, "tunnel_host_suffixes = [\"example.dev\"]\n").unwrap();
    assert_eq!(tunnel_host_suffixes(), vec!["example.dev".to_string()]);
}

#[test]
fn set_machine_preserves_tunnel_host_suffixes() {
    let home = temp_home("preserve");
    let _home = HomeGuard::set(&home);
    let config = machine_config_path();
    std::fs::create_dir_all(config.parent().unwrap()).unwrap();
    std::fs::write(&config, "tunnel_host_suffixes = [\"trycloudflare.com\"]\n").unwrap();
    set_machine("box-1").unwrap();
    assert_eq!(
        tunnel_host_suffixes(),
        vec!["trycloudflare.com".to_string()]
    );
    assert_eq!(read_machine_file().as_deref(), Some("box-1"));
}
