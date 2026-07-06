//! Read and sanitize the tail of a workbench `agent.log` for `svc_logs`/`svc_run_log`.

/// Max bytes of `agent.log` returned by `svc_logs` / `svc_run_log`. A long-running or noisy
/// agent can grow this file without bound; without a cap, serving the whole thing risks an
/// out-of-memory daemon and a multi-hundred-MB HTTP response for one request. Keeps only the
/// most recent bytes, since the tail is what matters for "what is this run doing right now".
pub(crate) const MAX_LOG_TAIL_BYTES: u64 = 2 * 1024 * 1024;

/// A log tail plus the metadata needed to tell "this is the whole file" from "this is a window"
/// (#280) — `total_bytes` is the full on-disk size and `truncated` is whether `content` was
/// capped to [`MAX_LOG_TAIL_BYTES`] rather than holding the complete file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LogWithMeta {
    pub(crate) content: String,
    pub(crate) total_bytes: u64,
    pub(crate) truncated: bool,
}

impl LogWithMeta {
    pub(crate) fn empty() -> Self {
        Self {
            content: String::new(),
            total_bytes: 0,
            truncated: false,
        }
    }
}

/// Same read as [`read_log_tail`], but reporting the full on-disk size and whether `content` is a
/// truncated window rather than the complete file (see [`LogWithMeta`]).
pub(crate) fn read_log_tail_with_meta(path: &std::path::Path) -> std::io::Result<LogWithMeta> {
    let total_bytes = std::fs::metadata(path)?.len();
    let content = read_log_tail(path)?;
    Ok(LogWithMeta {
        content,
        total_bytes,
        truncated: total_bytes > MAX_LOG_TAIL_BYTES,
    })
}

/// Read `path`, returning only the last [`MAX_LOG_TAIL_BYTES`] when it's larger than that.
///
/// The seek point is snapped forward to the next UTF-8 character boundary so a multi-byte
/// character split by the byte-offset seek isn't silently mangled, and a truncated read is
/// prefixed with a marker noting how many bytes were omitted rather than starting mid-line with
/// no indication anything is missing. [`strip_ansi_noise`] runs over the returned content so
/// terminal escape sequences and `\r`-redraw noise from the raw `tmux pipe-pane` capture (#278)
/// don't clutter the served log.
pub(crate) fn read_log_tail(path: &std::path::Path) -> std::io::Result<String> {
    let len = std::fs::metadata(path)?.len();
    if len <= MAX_LOG_TAIL_BYTES {
        return std::fs::read_to_string(path).map(|contents| strip_ansi_noise(&contents));
    }
    use std::io::{Read, Seek, SeekFrom};
    let omitted = len - MAX_LOG_TAIL_BYTES;
    let mut file = std::fs::File::open(path)?;
    file.seek(SeekFrom::Start(omitted))?;
    let mut buf = Vec::with_capacity(MAX_LOG_TAIL_BYTES as usize);
    file.read_to_end(&mut buf)?;
    // A UTF-8 continuation byte is 10xxxxxx; skip up to 3 of them (the longest possible
    // multi-byte sequence) to land on the next real character's leading byte.
    let start = buf
        .iter()
        .take(4)
        .position(|&byte| !(0x80..0xC0).contains(&byte))
        .unwrap_or(0);
    let tail = String::from_utf8_lossy(&buf[start..]);
    Ok(format!(
        "... [{omitted} bytes omitted; showing the last {MAX_LOG_TAIL_BYTES} bytes] ...\n{}",
        strip_ansi_noise(&tail)
    ))
}

/// Strip raw `tmux pipe-pane` capture noise from `input`: ANSI/VT escape sequences (color codes,
/// cursor movement, screen clears) and `\r`-based redraw overwrites from full-screen TUI agents.
///
/// `tmux pipe-pane -o` streams the pane's raw output verbatim, so a served log otherwise shows
/// escape codes as literal garbage and every redraw frame of a spinner/progress bar as a separate
/// line instead of the final, overwritten state a real terminal would display.
pub(crate) fn strip_ansi_noise(input: &str) -> String {
    let without_escapes = strip_escape_sequences(input);
    collapse_carriage_returns(&without_escapes)
}

/// Remove ANSI escape sequences: CSI (`ESC [ … final-byte`), OSC (`ESC ] … BEL` or `ESC ] … ESC \`),
/// and bare two-character escapes (e.g. `ESC c` full reset). A lone trailing `ESC` with no
/// follow-up byte is dropped as-is.
fn strip_escape_sequences(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    while let Some(current) = chars.next() {
        if current != '\u{1B}' {
            out.push(current);
            continue;
        }
        match chars.peek() {
            Some('[') => {
                chars.next();
                for pc in chars.by_ref() {
                    if ('@'..='~').contains(&pc) {
                        break;
                    }
                }
            }
            Some(']') => {
                chars.next();
                loop {
                    match chars.next() {
                        Some('\u{7}') | None => break,
                        Some('\u{1B}') => {
                            if chars.peek() == Some(&'\\') {
                                chars.next();
                            }
                            break;
                        }
                        Some(_) => {}
                    }
                }
            }
            Some(_) => {
                chars.next();
            }
            None => {}
        }
    }
    out
}

/// Collapse `\r`-based redraw overwrites: within each `\n`-delimited line, keep only the text
/// after the final `\r`, mirroring what a real terminal would leave on screen after a spinner or
/// progress bar repeatedly returns to the start of the line and overwrites itself.
fn collapse_carriage_returns(input: &str) -> String {
    input
        .split('\n')
        .map(|line| line.rsplit('\r').next().unwrap_or(line))
        .collect::<Vec<_>>()
        .join("\n")
}
