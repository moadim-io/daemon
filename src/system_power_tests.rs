use super::{env_override, is_system_power_saving_active};
use std::sync::{Mutex, MutexGuard};

static ENV_LOCK: Mutex<()> = Mutex::new(());

struct EnvGuard {
    _lock: MutexGuard<'static, ()>,
    power_saving: Option<String>,
    pmset_bin: Option<String>,
}

impl EnvGuard {
    fn set_power_saving(value: &str) -> Self {
        let lock = ENV_LOCK.lock().expect("env lock poisoned");
        let old = std::env::var("MOADIM_POWER_SAVING_ACTIVE").ok();
        let pmset_bin = std::env::var("MOADIM_PMSET_BIN").ok();
        // SAFETY: these tests serialize environment mutation with `ENV_LOCK` and restore it.
        unsafe {
            std::env::set_var("MOADIM_POWER_SAVING_ACTIVE", value);
        }
        Self {
            _lock: lock,
            power_saving: old,
            pmset_bin,
        }
    }

    #[cfg(target_os = "macos")]
    fn set_pmset_bin(value: &str) -> Self {
        let lock = ENV_LOCK.lock().expect("env lock poisoned");
        let power_saving = std::env::var("MOADIM_POWER_SAVING_ACTIVE").ok();
        let pmset_bin = std::env::var("MOADIM_PMSET_BIN").ok();
        // SAFETY: these tests serialize environment mutation with `ENV_LOCK` and restore it.
        unsafe {
            std::env::remove_var("MOADIM_POWER_SAVING_ACTIVE");
            std::env::set_var("MOADIM_PMSET_BIN", value);
        }
        Self {
            _lock: lock,
            power_saving,
            pmset_bin,
        }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        // SAFETY: this restores the env var snapshot captured by the test guard.
        unsafe {
            match &self.power_saving {
                Some(value) => std::env::set_var("MOADIM_POWER_SAVING_ACTIVE", value),
                None => std::env::remove_var("MOADIM_POWER_SAVING_ACTIVE"),
            }
            match &self.pmset_bin {
                Some(value) => std::env::set_var("MOADIM_PMSET_BIN", value),
                None => std::env::remove_var("MOADIM_PMSET_BIN"),
            }
        }
    }
}

#[test]
fn env_override_accepts_true_values() {
    let _guard = EnvGuard::set_power_saving("yes");

    assert_eq!(env_override(), Some(true));
}

#[test]
fn env_override_accepts_false_values() {
    let _guard = EnvGuard::set_power_saving("off");

    assert_eq!(env_override(), Some(false));
}

#[test]
fn env_override_ignores_unknown_values() {
    let _guard = EnvGuard::set_power_saving("maybe");

    assert_eq!(env_override(), None);
}

#[test]
fn top_level_detector_honors_true_env_override() {
    let _guard = EnvGuard::set_power_saving("true");

    assert!(is_system_power_saving_active());
}

#[test]
fn top_level_detector_honors_false_env_override() {
    let _guard = EnvGuard::set_power_saving("false");

    assert!(!is_system_power_saving_active());
}

#[cfg(target_os = "macos")]
#[test]
fn macos_detector_reads_battery_power_from_pmset() {
    let script = fake_pmset_script("echo \"Now drawing from 'Battery Power'\"");
    let _guard = EnvGuard::set_pmset_bin(script.to_str().expect("script path should be utf-8"));

    assert!(is_system_power_saving_active());
}

#[cfg(target_os = "macos")]
#[test]
fn macos_detector_reads_low_power_mode_from_pmset() {
    let script = fake_pmset_script(
        "if [ \"$2\" = \"batt\" ]; then echo \"Now drawing from 'AC Power'\"; else echo \" lowpowermode 1\"; fi",
    );
    let _guard = EnvGuard::set_pmset_bin(script.to_str().expect("script path should be utf-8"));

    assert!(is_system_power_saving_active());
}

#[cfg(target_os = "macos")]
#[test]
fn pmset_bin_defaults_to_pmset_when_unset() {
    let _guard = EnvGuard::set_pmset_bin("temporary");
    // SAFETY: this test owns the serialized environment guard.
    unsafe {
        std::env::remove_var("MOADIM_PMSET_BIN");
    }

    assert_eq!(super::pmset_bin(), "pmset");
}

#[cfg(target_os = "macos")]
#[test]
fn command_stdout_returns_none_for_failed_commands() {
    assert_eq!(super::command_stdout("/bin/sh", &["-c", "exit 7"]), None);
}

#[cfg(target_os = "macos")]
#[test]
fn command_stdout_returns_none_for_missing_commands() {
    assert_eq!(
        super::command_stdout("/definitely/missing/pmset", &[]),
        None
    );
}

#[cfg(target_os = "macos")]
fn fake_pmset_script(body: &str) -> std::path::PathBuf {
    use std::os::unix::fs::PermissionsExt;

    let path = std::env::temp_dir().join(format!(
        "moadim-fake-pmset-{}-{}.sh",
        std::process::id(),
        crate::utils::time::now_secs()
    ));
    std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).expect("write fake pmset script");
    let mut perms = std::fs::metadata(&path)
        .expect("stat fake pmset script")
        .permissions();
    perms.set_mode(0o700);
    std::fs::set_permissions(&path, perms).expect("chmod fake pmset script");
    path
}
