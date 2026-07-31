
#[tokio::test]
async fn cross_origin_get_is_not_rejected() {
    // Origin is only enforced on state-changing methods; a GET can't mutate anything.
    let resp = app()
        .oneshot(
            Request::builder()
                .uri("/")
                .header(header::HOST, "example.com")
                .header(header::ORIGIN, "http://attacker.example")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

struct EnvGuard {
    name: &'static str,
    previous: Option<std::ffi::OsString>,
}

impl EnvGuard {
    fn set(name: &'static str, value: &str) -> Self {
        let previous = std::env::var_os(name);
        // SAFETY: tests in this crate run single-threaded (`RUST_TEST_THREADS=1`, see
        // `.cargo/config.toml`), so no other thread observes the env in between.
        unsafe {
            std::env::set_var(name, value);
        }
        Self { name, previous }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        // SAFETY: single-threaded test execution, see `set` above.
        unsafe {
            match self.previous.take() {
                Some(value) => std::env::set_var(self.name, value),
                None => std::env::remove_var(self.name),
            }
        }
    }
}

#[test]
fn allowed_hosts_includes_loopback_defaults() {
    let hosts = allowed_hosts();
    assert!(hosts.iter().any(|host| host == "localhost"));
    assert!(hosts.iter().any(|host| host == "127.0.0.1"));
    assert!(hosts.iter().any(|host| host == "[::1]"));
}

#[test]
fn allowed_hosts_extends_from_env_var() {
    let _guard = EnvGuard::set(
        "MOADIM_ALLOWED_HOSTS",
        "reverse-proxy.internal, other.host:8080",
    );
    let hosts = allowed_hosts();
    assert!(hosts.iter().any(|host| host == "reverse-proxy.internal"));
    assert!(hosts.iter().any(|host| host == "other.host:8080"));
}

#[test]
fn allowed_hosts_skips_port_suffixed_entries_when_bind_addr_has_no_port() {
    // A bind address without a `:port` suffix (e.g. an operator setting `MOADIM_BIND_ADDR=0.0.0.0`
    // and letting the port default elsewhere) means `bind.rsplit_once(':')` returns `None`, so the
    // `localhost:<port>`/`127.0.0.1:<port>`/`[::1]:<port>` allowlist entries must not be added —
    // only the bare `bind` value itself.
    let _guard = EnvGuard::set("MOADIM_BIND_ADDR", "0.0.0.0");
    let hosts = allowed_hosts();
    assert!(hosts.iter().any(|host| host == "0.0.0.0"));
    assert!(!hosts.iter().any(|host| host.starts_with("localhost:")));
    assert!(!hosts.iter().any(|host| host.starts_with("127.0.0.1:")));
    assert!(!hosts.iter().any(|host| host.starts_with("[::1]:")));
}

#[test]
fn allowed_hosts_includes_port_suffixed_loopback_entries_when_bind_addr_has_port() {
    // A `Host` header carries the port a browser actually connected to (e.g.
    // `127.0.0.1:5784`), so every loopback spelling needs a `:<port>` counterpart alongside its
    // bare form — `127.0.0.1:<port>` was previously missing here even though `localhost:<port>`
    // and `[::1]:<port>` were both present, silently rejecting IPv4-loopback requests that
    // included the port.
    let _guard = EnvGuard::set("MOADIM_BIND_ADDR", "127.0.0.1:5784");
    let hosts = allowed_hosts();
    assert!(hosts.iter().any(|host| host == "localhost:5784"));
    assert!(hosts.iter().any(|host| host == "127.0.0.1:5784"));
    assert!(hosts.iter().any(|host| host == "[::1]:5784"));
}
