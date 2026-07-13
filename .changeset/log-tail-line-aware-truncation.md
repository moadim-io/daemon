---
"moadim": patch
---

fix(logs): snap truncated tail reads to the next line start

Prevents `--tail` reads that begin mid-line (because the read window doesn't align to
a line boundary) from emitting a partial first line. The read now skips ahead to the
next newline before returning output.
