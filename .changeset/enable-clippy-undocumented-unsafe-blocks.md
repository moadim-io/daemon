---
"moadim": patch
---

chore(lint): enable `clippy::undocumented_unsafe_blocks` workspace-wide, requiring a reasoned `// SAFETY:` comment for every `unsafe` block. This locks in the existing convention as a compiler-checked rule instead of an unenforced habit.
