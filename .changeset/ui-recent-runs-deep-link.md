---
"moadim": patch
---

fix(ui): deep-link RECENT RUNS entries straight to that routine's HISTORY page

Clicking a routine name in the overview page's RECENT RUNS panel used to
land on the plain routine list. It now carries a `?history=<id>` query
that the routines page reads on mount and opens that routine's HISTORY
page directly — one click from "what just ran, fleet-wide" to the full
per-run detail.
