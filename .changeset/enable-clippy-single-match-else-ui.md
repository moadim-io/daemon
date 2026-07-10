---
"moadim": patch
---

chore(lint): enable `clippy::single_match_else` in the `ui` crate

Adds `single_match_else = "deny"` to `ui/Cargo.toml`'s `[lints.clippy]` table, mirroring the
root crate's `single_match_else = "deny"` (`Cargo.toml`). The `ui` crate has its own
`[lints.clippy]` table with no `workspace = true` inheritance, so this lint (like several others
before it) silently never applied to `ui/src` despite CI's `clippy` job running `--workspace`.

`single_match_else` catches a `match` whose only non-wildcard arm destructures a single pattern,
with everything else falling to a catch-all arm — `if let ... else` says the same thing without
the unused generality of `match`, keeping a two-way branch as readable as a plain `if`.

The `ui` crate is already clean under this lint, so no code changes are needed — `deny` just
locks that state in. No behavior change.
