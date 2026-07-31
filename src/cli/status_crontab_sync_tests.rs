//! Tests for `moadim status` crontab-sync fields and recovery hints.

use std::io::{Read as _, Write as _};
use std::net::TcpListener;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use super::*;

struct EnvGuard {
    name: &'static str,
    previous: Option<std::ffi::OsString>,
}

impl EnvGuard {
    fn set(name: &'static str, value: &str) -> Self {
        let previous = std::env::var_os(name);
        // SAFETY: tests in this crate run single-threaded per binary.
        unsafe {
            std::env::set_var(name, value);
        }
        Self { name, previous }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        // SAFETY: single-threaded test execution.
        unsafe {
            match self.previous.take() {
                Some(value) => std::env::set_var(self.name, value),
                None => std::env::remove_var(self.name),
            }
        }
    }
}

struct FakeServer {
    addr: String,
    stop: Arc<AtomicBool>,
    handle: Option<std::thread::JoinHandle<()>>,
}

impl FakeServer {
    fn start(body: String) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind ephemeral port");
        let addr = listener.local_addr().expect("local addr").to_string();
        listener.set_nonblocking(true).expect("set nonblocking");
        let stop = Arc::new(AtomicBool::new(false));
        let stop_loop = Arc::clone(&stop);
        let handle = std::thread::spawn(move || {
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            while !stop_loop.load(Ordering::SeqCst) {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        let mut buf = [0_u8; 1024];
                        let _ = stream.read(&mut buf);
                        let _ = stream.write_all(response.as_bytes());
                    }
                    Err(ref err) if err.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(2));
                    }
                    Err(_) => break,
                }
            }
        });
        Self {
            addr,
            stop,
            handle: Some(handle),
        }
    }
}

impl Drop for FakeServer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

#[test]
fn status_json_includes_crontab_sync_recovery_hint_when_stale() {
    let health = HealthInfo {
        uptime_secs: 8123,
        version: "1.2.3".to_string(),
        crontab_sync: Some(CrontabSyncInfo {
            ok: false,
            last_error: Some("crontab: crontab - timed out after 15s".to_string()),
            last_error_at: Some(1_785_000_000),
        }),
    };
    let value: serde_json::Value =
        serde_json::from_str(&status_json(true, Some(42), Some(&health))).unwrap();
    assert_eq!(value["crontab_sync"]["ok"], serde_json::json!(false));
    assert_eq!(
        value["crontab_sync"]["last_error"],
        serde_json::json!("crontab: crontab - timed out after 15s")
    );
    assert_eq!(
        value["crontab_sync"]["recovery_hint"],
        serde_json::json!(CRONTAB_SYNC_RECOVERY_HINT)
    );
}

#[test]
fn status_json_omits_recovery_hint_when_crontab_sync_is_healthy() {
    let health = HealthInfo {
        uptime_secs: 1,
        version: "1.2.3".to_string(),
        crontab_sync: Some(CrontabSyncInfo {
            ok: true,
            last_error: None,
            last_error_at: None,
        }),
    };
    let value: serde_json::Value =
        serde_json::from_str(&status_json(true, Some(42), Some(&health))).unwrap();
    assert_eq!(value["crontab_sync"]["ok"], serde_json::json!(true));
    assert!(value["crontab_sync"]["recovery_hint"].is_null());
}

#[test]
fn parse_health_reads_crontab_sync_status() {
    let body = r#"{
        "uptime_secs":42,
        "version":"9.9.9",
        "crontab_sync":{
            "ok":false,
            "last_error":"crontab: blocked",
            "last_error_at":1785000000
        }
    }"#;
    assert_eq!(
        parse_health(body).and_then(|health| health.crontab_sync),
        Some(CrontabSyncInfo {
            ok: false,
            last_error: Some("crontab: blocked".to_string()),
            last_error_at: Some(1_785_000_000),
        })
    );
}

#[test]
fn parse_health_drops_malformed_crontab_sync_status() {
    let wrong_type = r#"{"uptime_secs":42,"version":"9.9.9","crontab_sync":{"ok":"no"}}"#;
    let missing_ok = r#"{"uptime_secs":42,"version":"9.9.9","crontab_sync":{}}"#;
    assert_eq!(parse_health(wrong_type).unwrap().crontab_sync, None);
    assert_eq!(parse_health(missing_ok).unwrap().crontab_sync, None);
}

#[test]
fn status_human_prints_crontab_sync_recovery_hint_when_stale() {
    let body = r#"{
        "uptime_secs":42,
        "version":"9.9.9",
        "crontab_sync":{"ok":false,"last_error":"crontab: blocked"}
    }"#;
    let server = FakeServer::start(body.to_string());
    let _addr = EnvGuard::set(BIND_ADDR_ENV, &server.addr);

    assert_eq!(status(false, None).unwrap(), 0);
}

#[test]
fn status_human_suppresses_crontab_sync_warning_when_healthy() {
    let body = r#"{
        "uptime_secs":42,
        "version":"9.9.9",
        "crontab_sync":{"ok":true}
    }"#;
    let server = FakeServer::start(body.to_string());
    let _addr = EnvGuard::set(BIND_ADDR_ENV, &server.addr);

    assert_eq!(status(false, None).unwrap(), 0);
}
