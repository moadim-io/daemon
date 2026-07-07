---
"moadim": patch
---

Deny `clippy::uninlined_format_args` in the `ui` crate, matching the root crate's lint config. The `ui` crate has its own `[lints.clippy]` table that doesn't inherit the root's deny-list, so this lint never applied to `ui/src` despite CI's `clippy` job running `--workspace`. The crate is already clean, so this locks it in.
