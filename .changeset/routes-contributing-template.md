---
"moadim": patch
---

docs(routes): add a template for logic/http/mcp endpoint folders

Adds `src/routes/CONTRIBUTING.md`, documenting the `mod.rs`/`logic.rs`/
`http.rs`/`mcp.rs` (+ `*_tests.rs` siblings) layout introduced by the
`routes/health/` refactor, so the next endpoint needing both a REST route
and an MCP tool over the same data has a copy-pasteable template — including
the `#[tool_router]`-splitting boilerplate (`vis = "pub(super)"`, the
parenthesized `Self::tool_router() + Self::<name>_tool_router()` router
combination, and the `__path_<name>` re-export utoipa needs) that isn't
obvious from reading `health/` alone. Root `CONTRIBUTING.md` now links to it
from the "Code conventions" section.

Docs only, no code change.
