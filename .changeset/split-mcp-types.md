---
"moadim": patch
---

### Fixed

Split the MCP tool input structs out of `src/routes/mcp.rs` into a new
`src/routes/mcp_types.rs` sibling module. `mcp.rs` had crept to 514 lines,
tripping the pre-push hook's 500-line-per-file gate (`linecheck --max-lines
500`) for every contributor who has `linecheck` installed, as CONTRIBUTING.md
instructs. No behavior change.
