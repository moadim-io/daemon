
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

/// A throwaway loopback HTTP server for driving the CLI's probe/signal client.
/// It answers every connection with a canned status line and body.
struct FakeServer {
    /// The `host:port` the server is listening on, for `MOADIM_BIND_ADDR`.
    addr: String,
    /// Signals the accept loop to exit.
    stop: Arc<AtomicBool>,
    /// The accept-loop thread handle, joined on drop.
    handle: Option<std::thread::JoinHandle<()>>,
}

impl FakeServer {
    /// Start a server on an ephemeral port answering with `status` and `body`.
    fn start(status: u16, body: String) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind ephemeral port");
        let addr = listener.local_addr().expect("local addr").to_string();
        listener.set_nonblocking(true).expect("set nonblocking");
        let stop = Arc::new(AtomicBool::new(false));
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

#[path = "bind_tests.rs"]
mod cli_bind_tests;

#[path = "query_tests.rs"]
mod cli_query_tests;

#[path = "bind_override_tests.rs"]
mod cli_bind_override_tests;

#[path = "restart_tests.rs"]
mod cli_restart_tests;

#[allow(
    clippy::missing_docs_in_private_items,
    reason = "split-out module keeps the file under the linecheck limit"
)]
#[path = "tests_part2.rs"]
mod tests_part2;

#[allow(
    clippy::missing_docs_in_private_items,
    reason = "split-out module keeps the file under the linecheck limit"
)]
#[path = "tests_part3.rs"]
mod tests_part3;
