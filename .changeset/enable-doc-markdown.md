---
"moadim": patch
---

chore(lint): enable `clippy::doc_markdown`

Requires backticks around code-like items (type names, paths, identifiers)
in doc comments, so they render as code in generated docs instead of plain
prose. Fixed the one violation this newly-`deny`d lint caught.
