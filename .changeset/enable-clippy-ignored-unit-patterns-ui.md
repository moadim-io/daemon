---
"moadim": patch
---

chore(lint): enable `clippy::ignored_unit_patterns` in the `ui` crate

The `ui` crate has its own `[lints.clippy]` table and doesn't inherit the root crate's extended
deny-list, so `ignored_unit_patterns` (already `deny`d root-side, #1200) never applied to
`ui/src` despite CI's `clippy` job running `--workspace`. Enabling it surfaced 35 violations
across `main.rs`, `routines/page.rs`, `routines/hooks.rs`, `routines/bulk_actions.rs`,
`schedule_heatmap.rs`, `overview.rs`, `settings.rs`, `refresh.rs`, `machines.rs`, and
`routines/form.rs`: `use_effect_with((), move |_| ...)` hooks and `Callback::from(move |_: ()|
...)` handlers all matched the `()`-typed argument with `_`, discarding its type instead of
stating it explicitly. Applied via `cargo clippy --fix`, rewriting each `_`/`_: ()` to `()`. No
behavior change.
