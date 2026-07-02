---
"moadim": patch
---

Enable `clippy::use_self` (issue #724) and replace the flagged `Type::Variant` spellings with `Self::Variant` inside their own `impl`/`match` blocks across `src/error.rs`, `src/machine/mod.rs`, `src/routines/agents/mod.rs`, `src/routines/flags.rs`, `src/sync/mod.rs`, and their test modules — purely mechanical, no behavior change.
