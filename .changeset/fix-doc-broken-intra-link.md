---
"moadim": patch
---

### Fixed

- **`cargo doc` no longer fails on `main`.** The doc comment on `sh_bin()` in
  `src/routines/service.rs` used an intra-doc link
  (`` [`crate::sync::crontab_bin`] ``) to a private, unexported function,
  which rustdoc can never resolve even with `--document-private-items`. This
  tripped `#![deny(warnings)]` and broke the `cargo doc` CI check (and any
  local `cargo doc` / `cargo install moadim` doc build) on every PR
  regardless of what it touched. Replaced the broken link with plain text.
