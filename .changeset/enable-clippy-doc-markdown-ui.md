---
"moadim": patch
---

chore(lint): enable clippy::doc_markdown in the ui crate

Mirrors the root crate's `doc_markdown = "deny"` (see `Cargo.toml`). The `ui` crate has its
own `[lints.clippy]` table and doesn't inherit root's extended deny-list, so this never
applied to `ui/src` despite CI's `clippy` job running `--workspace`. Fixed the 6 violations
this surfaced across `cron_utils.rs`, `routines/banner.rs`, `routines/filter.rs`,
`routines/filter_bar.rs`, `routines/filter_tests.rs`, and `routines/hooks.rs` by wrapping
the flagged identifiers (`is_valid`, `DueSoon`, `schedule_description`, `NodeRef`) in
backticks. No behavior change.
