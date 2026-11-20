//! `tunnel_host_suffixes` persistence tests: the key is read per call and survives other writes.

use super::*;

/// Serializes tests that change `MOADIM_HOME_OVERRIDE`, since the tests run in parallel.
static HOME_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Point `MOADIM_HOME_OVERRIDE` at a temp home for one test, restoring the prior value on drop.
pub(crate) struct HomeGuard {
    previous: Option<std::ffi::OsString>,
    _lock: std::sync::MutexGuard<'static, ()>,
}

impl HomeGuard {
    pub(crate) fn set(home: &std::path::Path) -> Self {
        let lock = HOME_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let previous = std::env::var_os("MOADIM_HOME_OVERRIDE");
        // SAFETY: env changes are serialized by HOME_LOCK, held for the guard's lifetime.
        unsafe { std::env::set_var("MOADIM_HOME_OVERRIDE", home) }
        Self {
            previous,
            _lock: lock,
        }
    }
}

impl Drop for HomeGuard {
    fn drop(&mut self) {
        // SAFETY: env changes are serialized by HOME_LOCK, still held here.
        unsafe {
            match self.previous.take() {
                Some(value) => std::env::set_var("MOADIM_HOME_OVERRIDE", value),
                None => std::env::remove_var("MOADIM_HOME_OVERRIDE"),
            }
        }
    }
}

pub(crate) fn temp_home(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("moadim-tunnel-{tag}-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).expect("create temp home");
    dir
}

#[tokio::test]
async fn tunnel_host_suffixes_absent_is_empty() {
    let home = temp_home("absent");
    let _home = HomeGuard::set(&home);
    assert!(tunnel_host_suffixes().await.is_empty());
}

#[tokio::test]
async fn tunnel_host_suffixes_read_from_file_each_call() {
    let home = temp_home("read");
    let _home = HomeGuard::set(&home);
    let config = machine_config_path();
    std::fs::create_dir_all(config.parent().unwrap()).unwrap();
    std::fs::write(&config, "tunnel_host_suffixes = [\"trycloudflare.com\"]\n").unwrap();
    assert_eq!(
        tunnel_host_suffixes().await,
        vec!["trycloudflare.com".to_string()]
    );

    std::fs::write(&config, "tunnel_host_suffixes = [\"example.dev\"]\n").unwrap();
    assert_eq!(
        tunnel_host_suffixes().await,
        vec!["example.dev".to_string()]
    );
}

#[tokio::test]
async fn set_machine_preserves_tunnel_host_suffixes() {
    let home = temp_home("preserve");
    let _home = HomeGuard::set(&home);
    let config = machine_config_path();
    std::fs::create_dir_all(config.parent().unwrap()).unwrap();
    std::fs::write(&config, "tunnel_host_suffixes = [\"trycloudflare.com\"]\n").unwrap();
    set_machine("box-1").unwrap();
    assert_eq!(
        tunnel_host_suffixes().await,
        vec!["trycloudflare.com".to_string()]
    );
    assert_eq!(read_machine_file().as_deref(), Some("box-1"));
}

#[tokio::test]
async fn unsafe_suffixes_are_dropped_at_load() {
    let home = temp_home("unsafe");
    let _home = HomeGuard::set(&home);
    let config = machine_config_path();
    std::fs::create_dir_all(config.parent().unwrap()).unwrap();
    std::fs::write(
        &config,
        "tunnel_host_suffixes = [\"com\", \"local\", \"\", \"  \", \"*.trycloudflare.com\", \"trycloudflare.com.\", \" trycloudflare.com\", \"a..b\"]\n",
    )
    .unwrap();
    assert!(tunnel_host_suffixes().await.is_empty());
}

#[tokio::test]
async fn valid_suffixes_are_kept_next_to_unsafe_ones() {
    let home = temp_home("mixed");
    let _home = HomeGuard::set(&home);
    let config = machine_config_path();
    std::fs::create_dir_all(config.parent().unwrap()).unwrap();
    std::fs::write(
        &config,
        "tunnel_host_suffixes = [\"com\", \"trycloudflare.com\", \"*.bad.dev\", \".example.dev\"]\n",
    )
    .unwrap();
    assert_eq!(
        tunnel_host_suffixes().await,
        vec!["trycloudflare.com".to_string(), ".example.dev".to_string()]
    );
}

#[tokio::test]
async fn malformed_toml_yields_no_suffixes() {
    let home = temp_home("malformed");
    let _home = HomeGuard::set(&home);
    let config = machine_config_path();
    std::fs::create_dir_all(config.parent().unwrap()).unwrap();
    std::fs::write(&config, "tunnel_host_suffixes = [\"trycloudflare.com\"\n").unwrap();
    assert!(tunnel_host_suffixes().await.is_empty());
}

#[tokio::test]
async fn string_instead_of_array_yields_no_suffixes() {
    let home = temp_home("wrong-type");
    let _home = HomeGuard::set(&home);
    let config = machine_config_path();
    std::fs::create_dir_all(config.parent().unwrap()).unwrap();
    std::fs::write(&config, "tunnel_host_suffixes = \"trycloudflare.com\"\n").unwrap();
    assert!(tunnel_host_suffixes().await.is_empty());
}

#[test]
fn wrong_typed_suffix_key_keeps_other_machine_keys() {
    let home = temp_home("wrong-type-keeps-name");
    let _home = HomeGuard::set(&home);
    let config = machine_config_path();
    std::fs::create_dir_all(config.parent().unwrap()).unwrap();
    std::fs::write(&config, "name = \"keep-me\"\ntunnel_host_suffixes = 7\n").unwrap();
    assert_eq!(read_machine_file().as_deref(), Some("keep-me"));
    assert!(read_machine_toml().tunnel_host_suffixes.is_empty());
}
