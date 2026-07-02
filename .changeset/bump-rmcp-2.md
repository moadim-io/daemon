---
"moadim": patch
---

### Changed

- **Bumped `rmcp` from 1.7.0 to 2.0.0.** The MCP SDK's `Content`/`RawContent`
  wrapper was replaced by the flat `ContentBlock` enum
  (`Text`/`Image`/`Audio`/`Resource`/`ResourceLink`); the tool-result
  constructors and test assertions were updated to match the new API. No
  behavioral change for MCP clients.
