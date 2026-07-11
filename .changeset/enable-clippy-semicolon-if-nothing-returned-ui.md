---
"moadim": patch
---

chore(lint): enable `clippy::semicolon_if_nothing_returned` in the `ui` crate

Adds `semicolon_if_nothing_returned = "deny"` to `ui/Cargo.toml`'s `[lints.clippy]` table,
matching the lint already denied workspace-root-side in `Cargo.toml` — the `ui` crate has its
own `[lints]` table (no `workspace = true` inheritance) so it silently escaped this despite CI's
`clippy` job running `--workspace`. Fixes the 10 violations this surfaced: `Callback::from`
closures in `main.rs`, `routines/actions.rs`, and `routines/page.rs` whose body was a bare
`spawn_local(...)`/`toast.emit(...)` call with no trailing semicolon, which read like the block
was returning that call's value even though the callback discards it. All fixes are a mechanical
added `;` — no behavior change. `prebuilt.html` is regenerated to match.
