---
"moadim": patch
---

fix(coverage): close the 100%-line-coverage gap left by 3 untestable error branches

Pre-existing on `main` (`cargo llvm-cov (100% line floor)` CI red for 3+ pushes), surfaced while
fixing `client (vitest)` (see the other changeset in this PR) — `std::env::current_exe()` failing
is otherwise unreachable in a test (the syscall only errors if the running binary's own file was
deleted mid-execution, or under unusual sandboxing), and re-serializing a `serde_json::Value` just
parsed from valid JSON text is unreachable too (the only failure mode is a non-finite float, which
JSON's grammar cannot express).

Added two test-only env-var seams, mirroring the existing `MOADIM_CRONTAB_BIN`/
`MOADIM_LAUNCHCTL_BIN` pattern for external-binary resolution:

- `utils::process::current_exe()` wraps `std::env::current_exe`, honoring
  `MOADIM_CURRENT_EXE_FAIL_FOR_TEST` in test builds. Used by `service::common::moadim_exe` and
  `cli::system::spawn_detached_with` (both call sites needed their own test, since the generic
  `spawn_detached_with`'s error-mapping closure is monomorphized separately per caller).
- `utils::claude_json::serialize_document()` wraps `serde_json::to_vec`, honoring
  `MOADIM_CLAUDE_JSON_SERIALIZE_FAIL_FOR_TEST`.

No behavior change outside `#[cfg(test)]`.
