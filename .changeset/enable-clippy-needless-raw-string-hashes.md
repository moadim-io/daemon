---
"moadim": patch
---

chore(lint): enable `clippy::needless_raw_string_hashes` in the root crate

Adds `needless_raw_string_hashes = "deny"` to `Cargo.toml`'s `[lints.clippy]` table and drops the
unneeded `#` delimiters from the one raw string literal that had them
(`src/routines/command_system_prompt.rs`) — its body contains no unescaped `"`, so the hashes
were pure noise. No behavior change.
