---
"moadim": patch
---

chore(lint): enable `clippy::todo` and `clippy::unimplemented`

Denies leftover `todo!()`/`unimplemented!()` stubs so they never ship to
production — a stray one would panic the daemon on that code path, same
rationale as the existing `dbg_macro` deny. Zero violations found; no code
changes needed.
