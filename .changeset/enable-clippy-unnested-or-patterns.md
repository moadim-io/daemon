---
"moadim": patch
---

chore(lint): enable `clippy::unnested_or_patterns` workspace-wide

Rejects an or-pattern repeated across multiple match arms/parameters instead of merged into a
single nested or-pattern, so duplicated arm bodies can't drift out of sync as arms are added or
reordered. The codebase was already clean, so no source changes were needed — `deny` just locks
that in.
