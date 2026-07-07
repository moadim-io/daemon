---
"moadim": patch
---

chore(lint): enable `clippy::needless_collect`

Locks in the codebase's existing zero-violation state for `clippy::needless_collect`, so a future PR that collects an iterator into a `Vec`/collection only to immediately re-iterate it (or check its length/emptiness) fails CI instead of shipping needless allocation overhead. No behavior change.
