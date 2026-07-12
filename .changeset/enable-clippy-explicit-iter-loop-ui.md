---
"moadim": patch
---

chore(lint): enable `clippy::explicit_iter_loop` in the `ui` crate

Adds `explicit_iter_loop = "deny"` to `ui/Cargo.toml`'s `[lints.clippy]` table, matching the lint
already denied workspace-root-side in `Cargo.toml` — the `ui` crate has its own `[lints]` table
(no `workspace = true` inheritance) so it silently escaped this despite CI's `clippy` job running
`--workspace`. Unlike most sibling `ui`-crate lint-enablement PRs, this one wasn't a no-op: it
surfaced two real violations in `day_timeline.rs`, rewritten from `for it in props.items.iter()`
and `for b in buckets.iter_mut()` to `for it in &props.items` and `for b in &mut buckets`. No
behavior change.
