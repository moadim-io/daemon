---
"moadim": patch
---

fix(cli): don't panic when writing a loopback HTTP request fails

`http_request_core` (`src/cli/system.rs`) used `.expect(...)` on `TcpStream::write_all`, even
though the very next line already tolerates a failed read on the same socket (a server that
closes the connection mid-request, e.g. while `moadim restart` is killing the old process).
Every caller (`status`, `stop`, `trigger`, `cleanup`, ...) already matches on this function's
`io::Result` to degrade gracefully to "moadim is not running" — the write failure just needs
to flow through the same `?` instead of panicking the CLI.
