---
"moadim": patch
---

Enable `clippy::unnecessary_debug_formatting` and fix the three flagged `log::warn!` call sites (`routine_storage::migrate_prompt_files_from_dir`, `routines::agents::ensure_default_agents_in`) that Debug-formatted (`{path:?}`) a `Path`/`PathBuf` in a user-facing log line instead of using `.display()`, matching every other path already printed this way in the codebase.
