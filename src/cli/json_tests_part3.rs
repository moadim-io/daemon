
/// Sorted object keys of a `--json` formatter's output, for order-independent comparison against
/// [`shape_keys`].
fn actual_keys(json: &str) -> Vec<String> {
    let value: serde_json::Value = serde_json::from_str(json).expect("formatter emits valid JSON");
    let mut keys: Vec<String> = value
        .as_object()
        .expect("formatter emits a JSON object")
        .keys()
        .cloned()
        .collect();
    keys.sort();
    keys
}

#[test]
fn readme_status_json_shape_matches_actual_keys() {
    let mut documented = shape_keys(&readme_json_shape("status"));
    documented.sort();
    let health = HealthInfo {
        uptime_secs: 42,
        version: "0.1.0".to_string(),
    };
    assert_eq!(
        documented,
        actual_keys(&status_json(true, Some(7), Some(&health))),
        "README `moadim status --json` shape has drifted from status_json's actual keys"
    );
}

#[test]
fn readme_cleanup_json_shape_matches_actual_keys() {
    let mut documented = shape_keys(&readme_json_shape("cleanup"));
    documented.sort();
    assert_eq!(
        documented,
        actual_keys(&cleanup_json(3, 12345, true)),
        "README `moadim cleanup --json` shape has drifted from cleanup_json's actual keys"
    );
}

#[test]
fn readme_stop_json_shape_matches_actual_keys() {
    let mut documented = shape_keys(&readme_json_shape("stop"));
    documented.sort();
    assert_eq!(
        documented,
        actual_keys(&stop_json(true, Some(7))),
        "README `moadim stop --json` shape has drifted from stop_json's actual keys"
    );
}

#[test]
fn print_help_and_version_emit_without_panicking() {
    print_help();
    print_version();
}

#[test]
fn stop_reports_not_running_when_no_server() {
    let home = temp_home("stop-down");
    let _home = EnvGuard::set("MOADIM_HOME_OVERRIDE", home.to_str().unwrap());
    let _addr = EnvGuard::set(BIND_ADDR_ENV, UNREACHABLE_ADDR);
    assert_eq!(stop(false, false).unwrap(), EXIT_NOT_RUNNING);
    assert_eq!(stop(true, false).unwrap(), EXIT_NOT_RUNNING);
    // --quiet suppresses the human line but keeps the exit-code contract.
    assert_eq!(stop(false, true).unwrap(), EXIT_NOT_RUNNING);
    let _ = std::fs::remove_dir_all(&home);
}

#[test]
fn stop_signals_running_server() {
    let server = FakeServer::start(200, String::new());
    let home = temp_home("stop-up");
    let _home = EnvGuard::set("MOADIM_HOME_OVERRIDE", home.to_str().unwrap());
    let _addr = EnvGuard::set(BIND_ADDR_ENV, &server.addr);
    assert_eq!(stop(false, false).unwrap(), 0);
    assert_eq!(stop(true, false).unwrap(), 0);
    // --quiet suppresses the human line but keeps the success exit code.
    assert_eq!(stop(false, true).unwrap(), 0);
    let _ = std::fs::remove_dir_all(&home);
}

#[allow(
    clippy::missing_docs_in_private_items,
    reason = "split-out module keeps the file under the linecheck limit"
)]
#[path = "json_tests_part2.rs"]
mod json_tests_part2;
