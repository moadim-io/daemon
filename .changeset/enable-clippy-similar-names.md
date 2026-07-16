---
"moadim": patch
---

chore(lint): enable `clippy::similar_names` workspace-wide

Rejects a binding whose name is a near-miss of another binding already in scope. Surfaced four
violations: `rmcp::model::ContentBlock::Text(txt) => txt.text.clone()`, repeated across the MCP
route tests, shadowed an existing local also named `text`. Renamed the match binding to `block`
in each spot. No behavior change.
