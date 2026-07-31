use std::io::{Read as _, Write as _};
use std::net::TcpListener;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use super::*;

/// Build a `Vec<String>` from string literals for [`parse`].
fn argv(args: &[&str]) -> Vec<String> {
    args.iter().map(ToString::to_string).collect()
}

pub(crate) const UNREACHABLE_ADDR: &str = "127.0.0.1:1";

pub(crate) struct EnvGuard {
    name: &'static str,
    previous: Option<std::ffi::OsString>,
}

impl EnvGuard {
    pub(crate) fn set(name: &'static str, value: &str) -> Self {
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

pub(crate) fn temp_home(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("moadim-cli-{tag}-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).expect("create temp home");
    dir
}

pub(crate) struct FakeServer {
    pub(crate) addr: String,
    alive: Arc<AtomicBool>,
    stop: Arc<AtomicBool>,
    handle: Option<std::thread::JoinHandle<()>>,
}

impl FakeServer {
    pub(crate) fn start(status: u16, body: String) -> Self {
        Self::start_with_liveness(status, body, true)
    }

    pub(crate) fn start_with_liveness(status: u16, body: String, initial_alive: bool) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind ephemeral port");
        let addr = listener.local_addr().expect("local addr").to_string();
        listener.set_nonblocking(true).expect("set nonblocking");
        let alive = Arc::new(AtomicBool::new(initial_alive));
        let stop = Arc::new(AtomicBool::new(false));
        let alive_loop = Arc::clone(&alive);
        let stop_loop = Arc::clone(&stop);
        let handle = std::thread::spawn(move || {
            let response = format!(
                "HTTP/1.1 {status} OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            while !stop_loop.load(Ordering::SeqCst) {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        let mut buf = [0_u8; 1024];
                        let _ = stream.read(&mut buf);
                        if alive_loop.load(Ordering::SeqCst) {
                            let _ = stream.write_all(response.as_bytes());
                        }
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
            alive,
            stop,
            handle: Some(handle),
        }
    }

    fn stop_after(&self, delay: Duration) {
        let alive = Arc::clone(&self.alive);
        std::thread::spawn(move || {
            std::thread::sleep(delay);
            alive.store(false, Ordering::SeqCst);
        });
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
include!("pid_file_write_read_clear_roundtrip_tests.rs");
