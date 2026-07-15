---
"moadim": patch
---

chore(lint): enable `clippy::needless_pass_by_value` in the `ui` crate

The `ui` crate has its own `[lints.clippy]` table and doesn't inherit the root crate's extended
deny-list, so `needless_pass_by_value` (already `deny`d root-side) never applied to `ui/src`
despite CI's `clippy` job running `--workspace`. Enabling it surfaced 5 violations in
`routines/actions.rs` and `routines/bulk_actions.rs`: `install_crud_handlers` and
`install_bulk_handlers` took their `state`/`toast`/`now` Yew handles by value but only ever
`.clone()`d them into closures, never consuming the outer parameter itself. Changed the
parameters to references (and updated the single call site in `routines/page.rs` to pass
borrows instead of pre-cloning), removing the needless ownership transfer. No behavior change.
