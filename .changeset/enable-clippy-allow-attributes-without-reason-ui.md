---
"moadim": patch
---

chore(lint): enable `clippy::allow_attributes_without_reason` in the `ui` crate

Adds `allow_attributes_without_reason = "deny"` to `ui/Cargo.toml`'s `[lints.clippy]` table,
matching the lint already denied workspace-root-side in `Cargo.toml` — the `ui` crate has its
own `[lints]` table (no `workspace = true` inheritance) so it silently escaped this despite
CI's `clippy` job running `--workspace`. There are no `#[allow(...)]` attributes anywhere in
`ui/src` today, so the `ui` crate is already clean under this lint; `deny` just locks that state
in and keeps any future suppression documented with a reason. No behavior change.
