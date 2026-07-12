---
"ui": patch
---

chore(lint): enable `clippy::needless_raw_string_hashes` in the ui crate

Adds `needless_raw_string_hashes = "deny"` to `ui/Cargo.toml`'s `[lints.clippy]` table. Mirrors
the root crate's lint (enabled in Cargo.toml), which never applied to `ui/src` since the `ui`
crate has its own `[lints.clippy]` table with no inheritance from the root. The `ui` crate is
already clean under it, so `deny` locks that in. No behavior change.
