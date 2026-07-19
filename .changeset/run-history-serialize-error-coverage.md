---
"moadim": patch
---

fix(routines): fold `run_history`'s serialize failure into its existing best-effort append chain, closing the coverage gate's last `run_history.rs` gap. `append_persisted_run` logged and returned early on a `serde_json::to_string` failure via its own dedicated `match`, separate from the `Result` chain already covering directory-creation/open/write failures for the same best-effort append — and since `PersistedRun`'s fields can never actually fail to serialize, that separate branch was untestable, leaving 3 lines permanently below `cargo llvm-cov`'s 100% line floor. Folding it into the same chain (one log call, one failure path, matching the function's own doc comment) removes the untestable branch entirely instead of contriving a test for it. This was the last of three follow-ups named by #1268; the remaining two (`cli/system.rs`, `service/common.rs`, `utils/claude_json.rs`, one line each) are unrelated and left for their own PRs.
