---
"moadim": minor
---

Support glob-style wildcards in a routine's `machines` targeting list: an entry containing `*` now matches the resolved machine name as a glob (`*` standing for any run of characters) instead of requiring an exact string. `machines = ["*"]` runs a routine on any machine; `machines = ["box-*"]` matches a whole family without enumerating each name. Plain entries with no `*` still match by exact equality, unchanged. Closes #1393.
