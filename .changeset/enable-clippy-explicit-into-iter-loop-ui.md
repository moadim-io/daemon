---
"moadim": patch
---

chore(lint): enable `clippy::explicit_into_iter_loop` in the `ui` crate

Adds `explicit_into_iter_loop = "deny"` to `ui/Cargo.toml`'s `[lints.clippy]` table, matching the
lint already denied workspace-root-side in `Cargo.toml` — the `ui` crate has its own `[lints]`
table (no `workspace = true` inheritance) so it silently escaped this despite CI's `clippy` job
running `--workspace`. Companion to the just-enabled `explicit_iter_loop`, this one catches the
`.into_iter()` (as opposed to `.iter()`/`.iter_mut()`) form of a redundant explicit iterator call
in a `for` loop. There are no such calls anywhere in `ui/src` today, so the `ui` crate is already
clean under this lint; `deny` just locks that state in. No behavior change.
