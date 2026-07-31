
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
