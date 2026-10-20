#![allow(
    clippy::missing_docs_in_private_items,
    reason = "test helpers and fixtures do not need doc comments"
)]

use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;

use super::*;

/// A temp root with a `bin/` dir (fake executables) and an `agents/` dir (agent configs).
struct Fixture {
    root: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!("moadim-doctor-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(root.join("bin")).expect("create bin dir");
        std::fs::create_dir_all(root.join("agents")).expect("create agents dir");
        Self { root }
    }

    fn path(&self) -> String {
        self.root.join("bin").display().to_string()
    }

    fn agents(&self) -> PathBuf {
        self.root.join("agents")
    }

    /// Create an empty executable named `name` in the fake `bin/` dir.
    fn bin(&self, name: &str) {
        let file = self.root.join("bin").join(name);
        std::fs::write(&file, "").expect("write fake binary");
        std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o755))
            .expect("chmod fake binary");
    }

    /// Write `agents/<name>.toml` with `body`.
    fn agent(&self, name: &str, body: &str) {
        std::fs::write(self.agents().join(format!("{name}.toml")), body)
            .expect("write agent config");
    }

    /// A fixture where `tmux`, `python3` and `claude` all resolve, with one good `claude` agent.
    fn healthy() -> Self {
        let fixture = Self::new();
        fixture.bin("tmux");
        fixture.bin("python3");
        fixture.bin("claude");
        fixture.agent("claude", "command = \"claude\"\n");
        fixture
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn find<'a>(checks: &'a [Check], label: &str) -> &'a Check {
    checks
        .iter()
        .find(|check| check.label == label)
        .unwrap_or_else(|| panic!("no check labelled {label}"))
}

#[test]
fn all_checks_pass_on_healthy_host() {
    let fixture = Fixture::healthy();
    let checks = run_checks(&fixture.path(), &fixture.agents(), true);
    assert!(checks.iter().all(Check::passed), "{checks:?}");
    assert_eq!(exit_code(&checks), 0);
    assert_eq!(find(&checks, "tmux").render(), "ok   tmux");
    assert_eq!(find(&checks, "agent claude").render(), "ok   agent claude");
}

#[test]
fn tmux_missing_fails_with_message() {
    let fixture = Fixture::healthy();
    std::fs::remove_file(fixture.root.join("bin").join("tmux")).unwrap();
    let checks = run_checks(&fixture.path(), &fixture.agents(), true);
    assert_eq!(
        find(&checks, "tmux").render(),
        "FAIL tmux — not found on PATH (routine runs cannot launch)"
    );
    assert_eq!(exit_code(&checks), 1);
}

#[test]
fn python3_missing_fails() {
    let fixture = Fixture::healthy();
    std::fs::remove_file(fixture.root.join("bin").join("python3")).unwrap();
    let checks = run_checks(&fixture.path(), &fixture.agents(), true);
    assert!(!find(&checks, "python3").passed());
    assert_eq!(exit_code(&checks), 1);
}

#[test]
fn agent_command_missing_fails() {
    let fixture = Fixture::healthy();
    fixture.agent("codex", "command = \"codex\"\n");
    let checks = run_checks(&fixture.path(), &fixture.agents(), true);
    assert_eq!(
        find(&checks, "agent codex").render(),
        "FAIL agent codex — `codex` not found on PATH"
    );
    assert!(find(&checks, "agent claude").passed());
    assert_eq!(exit_code(&checks), 1);
}

#[test]
fn unparseable_agent_toml_fails_with_error() {
    let fixture = Fixture::healthy();
    fixture.agent("broken", "command = [\n");
    let checks = run_checks(&fixture.path(), &fixture.agents(), true);
    let broken = find(&checks, "agent broken");
    assert!(!broken.passed());
    assert!(broken.render().starts_with("FAIL agent broken — "));
    assert_eq!(exit_code(&checks), 1);
}

#[test]
fn daemon_unreachable_fails_with_hint() {
    let fixture = Fixture::healthy();
    let checks = run_checks(&fixture.path(), &fixture.agents(), false);
    assert_eq!(
        find(&checks, "daemon").render(),
        "FAIL daemon — not running; start it with: moadim"
    );
    assert_eq!(exit_code(&checks), 1);
}

#[test]
fn missing_agents_dir_adds_no_agent_checks() {
    let fixture = Fixture::healthy();
    let absent = fixture.root.join("no-such-agents-dir");
    let checks = run_checks(&fixture.path(), &absent, true);
    let labels: Vec<&str> = checks.iter().map(|check| check.label.as_str()).collect();
    assert_eq!(labels, ["tmux", "python3", "daemon"]);
    assert_eq!(exit_code(&checks), 0);
}

#[test]
fn non_toml_files_in_agents_dir_are_ignored() {
    let fixture = Fixture::healthy();
    std::fs::write(fixture.agents().join("notes.txt"), "not a config").unwrap();
    let checks = run_checks(&fixture.path(), &fixture.agents(), true);
    assert!(checks.iter().all(|check| !check.label.contains("notes")));
    assert_eq!(exit_code(&checks), 0);
}

#[test]
fn exit_code_is_zero_only_when_every_check_passed() {
    let pass = Check::require("ok-one", true, "unused");
    let fail = Check::require("bad-one", false, "gone");
    assert_eq!(exit_code(std::slice::from_ref(&pass)), 0);
    assert_eq!(exit_code(&[pass, fail]), 1);
    assert_eq!(exit_code(&[]), 0);
}

#[test]
fn run_returns_a_valid_exit_code() {
    assert!(matches!(run(), 0 | 1));
}

#[test]
fn unreadable_agent_config_fails() {
    let fx = Fixture::healthy();
    // A directory named like a config cannot be read as a file.
    std::fs::create_dir(fx.agents().join("broken.toml")).expect("create dir");
    let checks = run_checks(&fx.path(), &fx.agents(), true);
    assert!(!find(&checks, "agent broken").passed());
}

include!("doctor_setup_tests.rs");
