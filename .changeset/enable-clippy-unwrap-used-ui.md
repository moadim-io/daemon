---
"moadim": patch
---

chore(lint): enable `clippy::unwrap_used` in the `ui` crate

The root crate denies `clippy::unwrap_used` in production code so a panic can't kill the
long-running daemon process; `ui/Cargo.toml` never inherited it, so the same class of
unhandled panic could ship in the dashboard UI unchecked. Adds the lint to `ui/Cargo.toml`
and fixes the one existing violation in `shell_dialogs.rs` (a `serde_json::to_string` call
that cannot actually fail, now an `.expect()` with a reason instead of a bare `.unwrap()`).
No behavior change.
