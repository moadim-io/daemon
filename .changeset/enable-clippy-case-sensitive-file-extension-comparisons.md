---
"moadim": patch
---

chore(lint): enable `clippy::case_sensitive_file_extension_comparisons`

Locks in the codebase's existing (near-)zero-violation state for `clippy::case_sensitive_file_extension_comparisons`, so a future case-sensitive `.ends_with(".ext")` file-extension check fails CI instead of silently disagreeing with the case-insensitive filesystems (macOS, Windows) this daemon runs on. Fixes the two existing violations in `src/routines/flags.rs` and its tests by switching `is_safe_flag_filename` (and the test asserting its shape) from `ends_with(".md")` to `Path::extension()` compared with `eq_ignore_ascii_case`. No behavior change on case-sensitive filesystems (Linux); on case-insensitive ones, a flag file named e.g. `bug-123.MD` is now correctly recognized instead of rejected.
