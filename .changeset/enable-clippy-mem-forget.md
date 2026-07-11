---
"moadim": patch
---

chore(lint): enable `clippy::mem_forget` in the root crate

Adds `mem_forget = "deny"` to `Cargo.toml`'s `[lints.clippy]` table. In a long-running daemon,
a `std::mem::forget`'d value's `Drop` impl never runs — for file handles, locks, and other RAII
guards that's an indefinitely leaked descriptor/lock rather than a one-off leak in a short-lived
program. The single existing use (a test that manually closes a file descriptor and must stop
`File::drop` from closing it again) gets a documented `#[allow(clippy::mem_forget, reason = ...)]`
so the intent stays explicit. No behavior change.
