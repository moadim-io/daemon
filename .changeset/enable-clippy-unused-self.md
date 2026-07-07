---
"moadim": patch
---

chore(lint): enable `clippy::unused_self`

Enables `unused_self = "deny"` to reject a `&self` method that never reads `self`. The 3 existing
violations — `list_agents`, `get_lock_status`, and `restart` in `src/routes/mcp.rs` — are `#[tool_router]`
MCP tool handlers whose `&self` receiver is dictated by the framework's uniform `self.method(...)`
dispatch, not by need, so each gets a documented `#[allow(clippy::unused_self, reason = "...")]` instead
of a signature change. No behavior change.
