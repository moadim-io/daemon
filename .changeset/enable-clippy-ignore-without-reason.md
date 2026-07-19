---
"moadim": patch
---

chore(lint): enable `clippy::ignore_without_reason` in the root crate, forbidding a bare `#[ignore]` on a test with no explanation of why it's skipped. Same rationale as the existing `allow_attributes_without_reason` lint, applied to test-skipping instead of lint-suppression: a silently ignored test rots invisibly unless the reason is written down next to it (e.g. `#[ignore = "requires a live tmux session"]`). No `#[ignore]` exists in the codebase today, so this adds no diff beyond the lint config — it just locks in that any future one must justify itself.
