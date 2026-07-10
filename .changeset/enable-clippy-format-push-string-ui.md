---
"moadim": patch
---

chore(lint): enable `clippy::format_push_string` in the `ui` crate

Adds `format_push_string = "deny"` to `ui/Cargo.toml`'s `[lints.clippy]` table, mirroring the
root crate's `format_push_string = "deny"` (`Cargo.toml`). The `ui` crate has its own
`[lints.clippy]` table with no `workspace = true` inheritance, so this lint (like several others
before it) silently never applied to `ui/src` despite CI's `clippy` job running `--workspace`.

`format_push_string` catches `.push_str(&format!(...))`, which allocates a throwaway `String`
only to immediately copy its contents into the target and drop it — a real perf-adjacent gap, not
just a style one. `write!`/`writeln!` write straight into the existing buffer instead.

The `ui` crate is already clean under this lint, so no code changes are needed — `deny` just locks
that state in. No behavior change.
