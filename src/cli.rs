//! Command-line interface: run-mode selection and background-process lifecycle.
//!
//! The `moadim` binary runs an HTTP/MCP/UI server. By default it starts that server **detached in
//! the background** and returns control to the shell — you then manage it from the client (the
//! `/ui` "STOP" button) or with `moadim stop`. Pass `--interactive` to run it in the foreground
//! attached to the terminal instead (Ctrl-C to stop).

use std::io::{Read as _, Write as _};
use std::net::SocketAddr;
use std::time::Duration;

/// Address the server binds to and that the client talks to.
pub const BIND_ADDR: &str = "127.0.0.1:5784";

/// How long to wait when probing or signalling a running server over HTTP.
const PROBE_TIMEOUT: Duration = Duration::from_millis(750);

/// Environment marker set on the backgrounded child so it knows it was spawned by the launcher.
const DAEMONIZED_ENV: &str = "MOADIM_DAEMONIZED";

/// The action the user asked for on the command line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    /// Run the server in the foreground, attached to the terminal (interactive mode).
    Foreground,
    /// Spawn the server as a detached background process, then exit (the default, non-interactive).
    Background,
    /// Ask a running background server to stop.
    Stop,
    /// Report whether a server is currently running.
    Status,
    /// Print usage help.
    Help,
    /// Print the binary version.
    Version,
}

/// Parse CLI arguments (excluding the program name) into a [`Command`].
///
/// Unknown arguments fall back to [`Command::Help`] so the user sees usage rather than a silent
/// no-op. With no arguments the default is [`Command::Background`].
pub fn parse(args: impl IntoIterator<Item = String>) -> Command {
    let args: Vec<String> = args.into_iter().collect();
    match args.first().map(String::as_str) {
        None => Command::Background,
        Some("stop") => Command::Stop,
        Some("status") => Command::Status,
        Some("-h" | "--help" | "help") => Command::Help,
        Some("-V" | "--version" | "version") => Command::Version,
        Some("-i" | "--interactive" | "-f" | "--foreground") => Command::Foreground,
        Some("-b" | "--background" | "-d" | "--detach" | "--daemon") => Command::Background,
        Some(_) => Command::Help,
    }
}

/// Print usage help to stdout.
pub fn print_help() {
    println!(
        "moadim — cron/MCP/REST server with a web control panel\n\
         \n\
         USAGE:\n\
         \x20   moadim [MODE]\n\
         \x20   moadim <COMMAND>\n\
         \n\
         MODES:\n\
         \x20   (default)              start the server in the background and exit\n\
         \x20   -i, --interactive      run in the foreground, attached to the terminal (Ctrl-C to stop)\n\
         \x20   -b, --background       start the server detached in the background (explicit default)\n\
         \n\
         COMMANDS:\n\
         \x20   stop                   stop a running background server\n\
         \x20   status                 show whether a server is running\n\
         \x20   help, -h, --help       show this help\n\
         \x20   version, -V            show the version\n\
         \n\
         Once running, manage the server from the web client at http://{BIND_ADDR}/ui\n\
         (the STOP button) or with `moadim stop`."
    );
}

/// Print the binary version to stdout.
pub fn print_version() {
    println!("moadim {}", env!("CARGO_PKG_VERSION"));
}

/// Start the server as a detached background process and return immediately.
///
/// If a server is already responding on [`BIND_ADDR`], it is stopped and replaced with a fresh
/// process so each launch yields a clean instance.
pub fn run_background() -> anyhow::Result<()> {
    if is_running() {
        let pid = read_pid_file()
            .map(|p| format!(" (pid {p})"))
            .unwrap_or_default();
        println!("moadim is already running{pid}; stopping it to start a fresh instance");
        crate::restart::stop_running_and_wait()?;
    }
    let pid = spawn_detached()?;
    println!("moadim started in the background (pid {pid}) at http://{BIND_ADDR}");
    println!("  UI    http://{BIND_ADDR}/ui");
    println!("  stop  moadim stop   (or use the STOP button in the UI)");
    println!("  logs  {}", paths_daemon_log());
    Ok(())
}

/// Ask a running server to stop via the `/shutdown` route.
pub fn stop() -> anyhow::Result<()> {
    match http_request("POST", "/shutdown") {
        Ok(200) => {
            println!("moadim is shutting down");
            Ok(())
        }
        Ok(status) => {
            anyhow::bail!("unexpected response from server: HTTP {status}");
        }
        Err(_) => {
            println!("moadim is not running");
            Ok(())
        }
    }
}

/// Report whether a server is running, with its PID when known.
pub fn status() -> anyhow::Result<()> {
    if is_running() {
        let pid = read_pid_file()
            .map(|p| format!(" (pid {p})"))
            .unwrap_or_default();
        println!("moadim is running{pid} at http://{BIND_ADDR}");
    } else {
        println!("moadim is not running");
    }
    Ok(())
}

/// Write the current process PID into the pid file so `stop`/`status` and signals can find it.
pub fn write_pid_file() -> anyhow::Result<()> {
    let path = crate::paths::pid_file();
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    ensure_config_gitignore();
    std::fs::write(&path, std::process::id().to_string())?;
    Ok(())
}

/// Write a `.gitignore` into the config dir so generated runtime files (`*.pid`, `*.log`)
/// stay out of version control when users track `~/.config/moadim` in a dotfiles repo.
/// Best-effort: failure to write it is not fatal to starting the daemon.
fn ensure_config_gitignore() {
    let gitignore = crate::paths::config_gitignore_path();
    if !gitignore.exists() {
        let _ = std::fs::write(&gitignore, "*.pid\n*.log\n");
    }
}

/// Remove the pid file. Best-effort: a missing file is not an error.
pub fn clear_pid_file() {
    let _ = std::fs::remove_file(crate::paths::pid_file());
}

/// Read the PID recorded in the pid file, if present and parseable.
pub(crate) fn read_pid_file() -> Option<u32> {
    std::fs::read_to_string(crate::paths::pid_file())
        .ok()?
        .trim()
        .parse()
        .ok()
}

/// The daemon log path, rendered for display.
fn paths_daemon_log() -> String {
    crate::paths::daemon_log_file().display().to_string()
}

/// Returns `true` if a server answers `GET /health` on [`BIND_ADDR`].
pub(crate) fn is_running() -> bool {
    matches!(http_request("GET", "/health"), Ok(200))
}

/// Send a minimal HTTP/1.1 request to the local server and return the response status code.
pub(crate) fn http_request(method: &str, path: &str) -> std::io::Result<u16> {
    let addr: SocketAddr = BIND_ADDR
        .parse()
        .expect("BIND_ADDR is a valid socket address");
    let mut stream = std::net::TcpStream::connect_timeout(&addr, PROBE_TIMEOUT)?;
    stream.set_read_timeout(Some(PROBE_TIMEOUT))?;
    stream.set_write_timeout(Some(PROBE_TIMEOUT))?;
    let req = format!(
        "{method} {path} HTTP/1.1\r\nHost: {BIND_ADDR}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
    );
    stream.write_all(req.as_bytes())?;
    let mut resp = String::new();
    // A failed read after a clean shutdown can still yield the status line we already received.
    let _ = stream.read_to_string(&mut resp);
    parse_status_code(&resp).ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "no HTTP status line in response",
        )
    })
}

/// Extract the numeric status code from an HTTP response's status line (e.g. `HTTP/1.1 200 OK`).
fn parse_status_code(resp: &str) -> Option<u16> {
    resp.lines().next()?.split_whitespace().nth(1)?.parse().ok()
}

/// Spawn a detached copy of this binary running the server in the foreground, returning its PID.
///
/// The child runs with `--interactive` (so it actually serves), in its own process group so a
/// terminal SIGINT to the launcher does not reach it, with stdio redirected to the daemon log.
fn spawn_detached() -> anyhow::Result<u32> {
    use std::process::{Command as Proc, Stdio};

    let exe = std::env::current_exe()?;
    let log_path = crate::paths::daemon_log_file();
    if let Some(dir) = log_path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let out = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)?;
    let err = out.try_clone()?;

    let mut cmd = Proc::new(exe);
    cmd.arg("--interactive")
        .env(DAEMONIZED_ENV, "1")
        .stdin(Stdio::null())
        .stdout(Stdio::from(out))
        .stderr(Stdio::from(err));
    detach(&mut cmd);

    let child = cmd.spawn()?;
    Ok(child.id())
}

/// Put the spawned child in its own process group so it survives the launcher and terminal signals.
#[cfg(unix)]
fn detach(cmd: &mut std::process::Command) {
    use std::os::unix::process::CommandExt as _;
    cmd.process_group(0);
}

/// No-op on platforms without process groups; the child still detaches via redirected stdio.
#[cfg(not(unix))]
fn detach(_cmd: &mut std::process::Command) {}

#[cfg(test)]
#[path = "cli_tests.rs"]
mod cli_tests;
