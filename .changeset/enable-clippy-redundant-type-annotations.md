---
"moadim": patch
---

chore(lint): enable `clippy::redundant_type_annotations` workspace-wide

Rejects a `let` binding whose explicit type annotation exactly matches what the compiler would
already infer — the annotation adds nothing beyond what the initializer already states, so it's
noise a reader has to cross-check against the inferred type instead of trusting. Enabled in both
the root `Cargo.toml` and `ui/Cargo.toml` (the `ui` crate has its own `[lints.clippy]` table and
doesn't inherit root's deny-list), mirroring the existing lint-parity pattern.

The workspace (both `src/` and `ui/src`) was already clean, so no fixes were needed — `deny`
just locks that in.
