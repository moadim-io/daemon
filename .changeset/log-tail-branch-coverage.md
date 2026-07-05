---
"moadim": patch
---

test(routines): cover two untested branches in `service_log_tail.rs`

`cargo llvm-cov`'s region report (not the 100%-line gate, which region
coverage doesn't affect) showed two real gaps in the routine log-tail /
ANSI-sanitizing logic used by `svc_logs`/`svc_run_log`:

- `read_log_tail` never had a test for its very first fallible step
  (`std::fs::metadata(path)?`) — a workbench whose `agent.log` was removed
  out from under it (e.g. a racing cleanup sweep) must surface an
  `io::Error`, not panic.
- `strip_ansi_noise`'s OSC-sequence parser only had a test for the
  terminator `ESC \`; the other valid terminator, a bare `ESC` not
  followed by `\`, was never exercised, and it has different behavior
  (the character right after that `ESC` is not consumed, unlike the
  `ESC \` case).

No behavior change — regression tests only.
