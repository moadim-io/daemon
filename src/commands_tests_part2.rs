#![allow(
    clippy::wildcard_imports,
    clippy::too_many_arguments,
    clippy::needless_pass_by_value,
    dead_code,
    reason = "split-out module preserves existing code while keeping files under the linecheck limit"
)]
use super::*;

#[test]
fn every_subcommand_succeeds_against_a_2xx_server() {
    let server = FakeServer::start(200, "{\"ok\":true}");
    let _addr = EnvGuard::set(BIND_ENV, &server.addr);

    let calls: &[&[&str]] = &[
        // routines
        &[
            "routines",
            "create",
            "--schedule",
            "* * * * *",
            "--title",
            "t",
            "--agent",
            "a",
            "--prompt",
            "p",
        ],
        &[
            "routine",
            "create",
            "--schedule",
            "* * * * *",
            "--title",
            "t",
            "--agent",
            "a",
            "--model",
            "claude-sonnet-4-6",
            "--prompt",
            "p",
            "--disabled",
            "--repositories",
            "[]",
            "--tag",
            "triage",
            "--tag",
            "nightly",
        ],
        &["routines", "list"],
        &["routines", "get", "rid"],
        &[
            "routines",
            "update",
            "rid",
            "--schedule",
            "@hourly",
            "--title",
            "t2",
            "--model",
            "",
            "--repositories",
            "[]",
            "--enabled",
            "false",
            "--ttl-secs",
            "10",
            "--max-runtime-secs",
            "20",
            "--tag",
            "ops",
        ],
        &[
            "routines",
            "replace",
            "rid",
            "--schedule",
            "* * * * *",
            "--title",
            "t",
            "--agent",
            "a",
            "--prompt",
            "p",
        ],
        &["routines", "delete", "rid"],
        &[
            "routines",
            "move",
            "rid",
            "--folder",
            "maintenance",
            "--slug",
            "nightly",
        ],
        &["routines", "trigger", "rid"],
        &["routines", "logs", "rid"],
        &["routines", "ical"],
        // schedule (posts to the routine scheduled-trigger route)
        &["schedule", "trigger", "sid"],
        &["sched", "trigger", "sid"],
        // top-level
        &["agents"],
    ];
    for call in calls {
        assert_eq!(run(argv(call)), 0, "call {call:?}");
    }
}

#[test]
fn logs_print_raw_when_body_is_not_json() {
    let server = FakeServer::start(200, "plain log line\nsecond line");
    let _addr = EnvGuard::set(BIND_ENV, &server.addr);
    assert_eq!(run(argv(&["routines", "logs", "abc"])), 0);
}

#[test]
fn move_preserves_current_slug_when_slug_is_omitted() {
    let server = FakeServer::start(
        200,
        "{\"id\":\"rid\",\"slug\":\"daily\",\"rel_path\":\"maintenance/daily\"}",
    );
    let _addr = EnvGuard::set(BIND_ENV, &server.addr);
    assert_eq!(
        run(argv(&["routine", "move", "rid", "--folder", "maintenance"])),
        0
    );
}

#[test]
fn empty_body_prints_nothing_and_succeeds() {
    let server = FakeServer::start(200, "");
    let _addr = EnvGuard::set(BIND_ENV, &server.addr);
    assert_eq!(run(argv(&["agents"])), 0);
}

#[test]
fn non_2xx_status_returns_one() {
    // A non-empty error body exercises the "print the body" branch.
    {
        let server = FakeServer::start(404, "{\"error\":\"not found\"}");
        let _addr = EnvGuard::set(BIND_ENV, &server.addr);
        assert_eq!(run(argv(&["routines", "get", "missing"])), 1);
    }
    // An empty error body exercises the "skip the body" branch.
    {
        let server = FakeServer::start(500, "");
        let _addr = EnvGuard::set(BIND_ENV, &server.addr);
        assert_eq!(run(argv(&["routines", "list"])), 1);
    }
}

#[test]
fn no_server_returns_not_running_exit_code() {
    let _addr = EnvGuard::set(BIND_ENV, UNREACHABLE_ADDR);
    assert_eq!(
        run(argv(&["routines", "list"])),
        crate::cli::EXIT_NOT_RUNNING
    );
    // `schedule trigger` reaches the same not-running path.
    assert_eq!(
        run(argv(&["schedule", "trigger", "sid"])),
        crate::cli::EXIT_NOT_RUNNING
    );
}

#[test]
fn enable_disable_report_server_echoed_state() {
    // A 2xx whose body echoes the routine drives the "prefer the server's id/enabled" path, for
    // both states and both output modes (human line + --json object).
    {
        let server = FakeServer::start(200, "{\"id\":\"r-1\",\"enabled\":true}");
        let _addr = EnvGuard::set(BIND_ENV, &server.addr);
        assert_eq!(run(argv(&["enable", "r-1"])), 0);
        assert_eq!(run(argv(&["enable", "r-1", "--json"])), 0);
    }
    {
        let server = FakeServer::start(200, "{\"id\":\"r-1\",\"enabled\":false}");
        let _addr = EnvGuard::set(BIND_ENV, &server.addr);
        assert_eq!(run(argv(&["disable", "slug"])), 0);
        assert_eq!(run(argv(&["disable", "slug", "--json"])), 0);
    }
}

#[test]
fn enable_disable_fall_back_to_requested_state() {
    // A 2xx whose body lacks id/enabled (here: an empty JSON object, and a non-JSON body) exercises
    // the fallback to the addressed routine and the requested flag, for both states.
    {
        let server = FakeServer::start(200, "{}");
        let _addr = EnvGuard::set(BIND_ENV, &server.addr);
        assert_eq!(run(argv(&["enable", "slug"])), 0);
        assert_eq!(run(argv(&["disable", "slug", "--json"])), 0);
    }
    {
        let server = FakeServer::start(200, "not json");
        let _addr = EnvGuard::set(BIND_ENV, &server.addr);
        assert_eq!(run(argv(&["enable", "slug"])), 0);
    }
}

#[test]
fn enable_unknown_routine_returns_one() {
    // A non-empty error body exercises the "print the body" branch...
    {
        let server = FakeServer::start(404, "{\"error\":\"not found\"}");
        let _addr = EnvGuard::set(BIND_ENV, &server.addr);
        assert_eq!(run(argv(&["enable", "missing"])), 1);
    }
    // ...and an empty one the "skip the body" branch.
    {
        let server = FakeServer::start(500, "");
        let _addr = EnvGuard::set(BIND_ENV, &server.addr);
        assert_eq!(run(argv(&["disable", "missing"])), 1);
    }
}

#[test]
fn enable_without_server_returns_not_running() {
    let _addr = EnvGuard::set(BIND_ENV, UNREACHABLE_ADDR);
    assert_eq!(run(argv(&["enable", "r-1"])), crate::cli::EXIT_NOT_RUNNING);
}

#[test]
fn insert_opt_only_inserts_present_values() {
    let mut map = Map::new();
    insert_opt(&mut map, "a", Some(Value::Bool(true)));
    insert_opt(&mut map, "b", None);
    assert_eq!(map.get("a"), Some(&Value::Bool(true)));
    assert!(!map.contains_key("b"));
}
