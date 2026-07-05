---
"moadim": patch
---

chore(lint): enable `clippy::explicit_into_iter_loop`

Companion to the already-enabled `clippy::explicit_iter_loop`: rejects
`for x in collection.into_iter()` in favor of the equivalent, shorter
`for x in collection`. The workspace was already clean against this lint
(zero violations), so `deny` just locks that in. No behavior change.
