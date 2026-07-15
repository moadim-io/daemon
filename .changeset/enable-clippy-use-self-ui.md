---
"moadim": patch
---

chore(lint): enable clippy::use_self in the ui crate

Mirrors the root crate's `use_self = "deny"` (see `Cargo.toml`). The `ui` crate has its own
`[lints.clippy]` table and doesn't inherit root's extended deny-list, so this never applied
to `ui/src` despite CI's `clippy` job running `--workspace`. Fixed the 135 violations this
surfaced across `refresh.rs`, `routines/state.rs`, `routines/filter.rs`,
`overview_attention.rs`, and `schedule_heatmap_grid.rs` via `cargo clippy --fix`, replacing
enum/type name repetition (e.g. `RGroupBy::Status`) with `Self::Status` inside their own impl
blocks. No behavior change.
