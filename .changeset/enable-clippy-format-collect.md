---
"moadim": patch
---

chore(lint): enable `clippy::format_collect` workspace-wide

Mirrors the existing `format_push_string = "deny"` lint (root `Cargo.toml`), which rejects
`.push_str(&format!(...))` in favor of writing straight into the buffer with `write!`. This
adds its sibling for the `.map(|x| format!(...)).collect::<String>()` shape, which has the same
throwaway-allocation problem but wasn't yet covered. Surfaced one violation in
`src/routines/service_trigger_tests.rs`, rewritten to fold a `writeln!` directly into the
accumulator instead of collecting a `Vec` of one-off `format!` strings. No behavior change.
