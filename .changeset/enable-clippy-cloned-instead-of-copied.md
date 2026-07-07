---
"moadim": patch
---

chore(lint): enable `clippy::cloned_instead_of_copied`

Locks in the codebase's existing zero-violation state for `clippy::cloned_instead_of_copied`, so a future `.cloned()` call on an iterator/`Option` of a `Copy` type fails CI instead of shipping a needlessly indirect clone where `.copied()` says the same thing more directly. No behavior change.
