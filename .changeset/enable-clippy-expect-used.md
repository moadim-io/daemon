---
"moadim": patch
---

chore(lint): enable `clippy::expect_used` in the root crate, forbidding `.expect()` in production code. `.expect()` panics exactly like `.unwrap()` (already forbidden via `unwrap_used`) — it only adds a custom message to the same daemon-killing failure mode, so `unwrap_used` alone left it an unguarded back door. Fixed the 25 violations this surfaced: most now propagate a proper `Result` (`?`, `ok_or_else`, or a `let else` early return); a handful of genuinely provable "can't happen" invariants (an id checked to exist earlier in the same function with the lock held continuously since; `Stdio::piped()` set a few lines above the matching `.take()`) are kept as `.expect()` behind a scoped, reasoned `#[allow]`. `build.rs` and its `src/build/` helper modules carry their own crate-level exemption — a build script's `.expect()` panic just aborts `cargo build` with a message, the same intended failure mode as test code.
