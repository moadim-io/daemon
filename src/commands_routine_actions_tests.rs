//! Coverage tests for routine CLI actions that parse server responses.

use super::*;
use std::io::{Read as _, Write as _};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

const BIND_ENV: &str = "MOADIM_BIND_ADDR";

struct EnvGuard(Option<std::ffi::OsString>);

impl EnvGuard {
    fn set(value: &str) -> Self {
        let previous = std::env::var_os(BIND_ENV);
        // SAFETY: tests run single-threaded in this crate.
        unsafe { std::env::set_var(BIND_ENV, value) };
        Self(previous)
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        // SAFETY: tests run single-threaded in this crate.
        unsafe {
            match self.0.take() {
                Some(value) => std::env::set_var(BIND_ENV, value),
                None => std::env::remove_var(BIND_ENV),
            }
        }
    }
}

struct SequenceServer {
    addr: String,
    handle: Option<JoinHandle<()>>,
}

impl SequenceServer {
    fn start(responses: Vec<(u16, &'static str)>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind ephemeral port");
        let addr = listener.local_addr().expect("local addr").to_string();
        listener.set_nonblocking(true).expect("set nonblocking");
        let responses = Arc::new(Mutex::new(responses.into_iter()));
        let loop_responses = Arc::clone(&responses);
        let handle = std::thread::spawn(move || loop {
            match listener.accept() {
                Ok((mut stream, _)) => {
                    let mut buf = [0_u8; 2048];
                    let _ = stream.read(&mut buf);
                    let Some((status, body)) = loop_responses.lock().unwrap().next() else {
                        break;
                    };
                    let response = format!(
                            "HTTP/1.1 {status} OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                            body.len()
                        );
                    let _ = stream.write_all(response.as_bytes());
                }
                Err(ref err) if err.kind() == std::io::ErrorKind::WouldBlock => {
                    if loop_responses.lock().unwrap().as_slice().is_empty() {
                        break;
                    }
                    std::thread::sleep(Duration::from_millis(2));
                }
                Err(_) => break,
            }
        });
        Self {
            addr,
            handle: Some(handle),
        }
    }
}

impl Drop for SequenceServer {
    fn drop(&mut self) {
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

#[test]
fn move_resolves_slug_through_list_before_posting() {
    let server = SequenceServer::start(vec![
        (404, "{}"),
        (
            200,
            "[{\"id\":\"rid\",\"slug\":\"daily\",\"rel_path\":\"ops/daily\"}]",
        ),
        (200, "{\"id\":\"rid\",\"rel_path\":\"maintenance/daily\"}"),
    ]);
    let _addr = EnvGuard::set(&server.addr);
    assert_eq!(move_routine("daily", Some("maintenance".into()), None), 0);
}

#[test]
fn move_reports_resolution_failures() {
    let _addr = EnvGuard::set("127.0.0.1:1");
    assert_eq!(
        move_routine("missing", Some("ops".into()), None),
        crate::cli::EXIT_NOT_RUNNING
    );

    let missing = SequenceServer::start(vec![(404, "{}"), (200, "[]")]);
    let _addr = EnvGuard::set(&missing.addr);
    assert_eq!(move_routine("missing", Some("ops".into()), None), 1);

    let list_error = SequenceServer::start(vec![(404, "{}"), (500, "boom")]);
    let _addr = EnvGuard::set(&list_error.addr);
    assert_eq!(move_routine("missing", Some("ops".into()), None), 1);

    let list_down = SequenceServer::start(vec![(404, "{}")]);
    let _addr = EnvGuard::set(&list_down.addr);
    assert_eq!(
        move_routine("missing", Some("ops".into()), None),
        crate::cli::EXIT_NOT_RUNNING
    );

    let invalid = SequenceServer::start(vec![(404, "{}"), (200, "not json")]);
    let _addr = EnvGuard::set(&invalid.addr);
    assert_eq!(move_routine("missing", Some("ops".into()), None), 1);
}

#[test]
fn move_reports_post_failures_and_plain_success() {
    let conflict = SequenceServer::start(vec![
        (200, "{\"id\":\"rid\",\"slug\":\"daily\"}"),
        (409, "{\"error\":\"exists\"}"),
    ]);
    let _addr = EnvGuard::set(&conflict.addr);
    assert_eq!(move_routine("rid", Some("ops".into()), None), 1);

    let plain = SequenceServer::start(vec![(200, "{\"id\":\"rid\"}"), (200, "moved")]);
    let _addr = EnvGuard::set(&plain.addr);
    assert_eq!(
        move_routine("rid", Some("ops".into()), Some("daily".into())),
        0
    );
}

#[test]
fn move_surfaces_liveness_and_missing_slug_failures() {
    let get_only = SequenceServer::start(vec![(200, "{\"id\":\"rid\",\"slug\":\"daily\"}")]);
    let _addr = EnvGuard::set(&get_only.addr);
    assert_eq!(
        move_routine("rid", Some("ops".into()), None),
        crate::cli::EXIT_NOT_RUNNING
    );

    let no_slug = SequenceServer::start(vec![(200, "{\"ok\":true}")]);
    let _addr = EnvGuard::set(&no_slug.addr);
    assert_eq!(move_routine("rid", Some("ops".into()), None), 1);
}
