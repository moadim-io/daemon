---
"moadim": patch
---

Enable `clippy::manual_assert` workspace-wide, continuing this crate's incremental clippy-lint
enablement (see `enable-clippy-expect-used`, the `redundant_type_annotations`/
`unseparated_literal_suffix` lints, etc.). Rejects `if !cond { panic!("msg") }` in favour of
`assert!(cond, "msg")`, which states the invariant directly instead of making the reader invert
the condition. The codebase already had zero violations, so this is a lint-only change with no
behavior change.
