# Adding a route

Small routes (a couple of one-off handlers) can go straight into `http.rs` /
`mcp.rs`. Once a concept has both a REST endpoint *and* an MCP tool that
should return the same data, give it its own folder instead of duplicating
the response-building logic in two places. `src/routes/health/` is the
reference implementation — copy its shape.

## Layout

```
src/routes/<name>/
  mod.rs        — wiring only: declares the submodules, re-exports the public surface
  logic.rs      — response type(s) + the pure function that builds them (no framework code)
  http.rs       — the Axum handler, thin: extracts state, calls logic, wraps in Json
  http_tests.rs — tests for http.rs
  mcp.rs        — the MCP tool, thin: calls logic, wraps in the MCP result type
  mcp_tests.rs  — tests for mcp.rs
  logic_tests.rs — tests for logic.rs (the part worth testing directly, no HTTP/MCP plumbing)
```

Both `http.rs` and `mcp.rs` call the *same* `logic::build(...)` — that's the
whole point of the split. If the two surfaces genuinely need different
fields, add the field to the shared response type rather than hand-building
a second, divergent payload (see the `server_root`/`server_exe_dir` fields on
`HealthResponse` — the MCP tool doesn't get special-cased JSON, both surfaces
return the identical struct).

Tests live in `*_tests.rs` sibling files, never inline `#[cfg(test)] mod`
blocks — same rule as the rest of `src/`, see the root
[`CONTRIBUTING.md`](../../CONTRIBUTING.md#tests).

## `logic.rs`

```rust
//! Shared <name> logic: response shape and how to build it. Both the HTTP handler
//! (`http.rs`) and the MCP tool (`mcp.rs`) build on top of this.

use serde::Serialize;

/// Response body for `GET /<name>` and the `<name>` MCP tool.
#[derive(Serialize, utoipa::ToSchema)]
pub struct NameResponse {
    // ...fields...
}

/// Build the current `<name>` payload.
pub fn build(/* whatever inputs it needs */) -> NameResponse {
    NameResponse { /* ... */ }
}

#[cfg(test)]
#[path = "logic_tests.rs"]
mod logic_tests;
```

## `http.rs`

```rust
//! `GET /<name>` HTTP handler.

use super::logic;
use axum::{extract::State, Json};
use logic::NameResponse;

#[utoipa::path(get, path = "/<name>",
    responses((status = 200, body = NameResponse)))]
pub async fn name(State(state): State<crate::routes::http::AppState>) -> Json<NameResponse> {
    Json(logic::build(/* from state */))
}

#[cfg(test)]
#[path = "http_tests.rs"]
mod http_tests;
```

Wire it into the router in `routes/http.rs`:

```rust
use super::name; // the new module
// ...
.route("/<name>", get(name::name))
```

And into `openapi.rs`'s `paths(...)`/`components(schemas(...))` lists as
`crate::routes::<name>::name` / `crate::routes::<name>::NameResponse` — no
manual `__path_*` re-export needed there, since `openapi.rs` references the
handler at the same path where `#[utoipa::path]` generated it.

## `mcp.rs` — the part that isn't obvious

This file has to be declared as a **child module of `routes::mcp`**, not of
`routes::<name>`, even though it physically lives in `src/routes/<name>/`.
`MoadimMcp`'s fields (`routines`, `uptime_start`, `shutdown`, ...) and the
`ok()`/`err()` helpers are private to `routes::mcp` — a descendant module can
see them, a sibling can't. Rust's `#[path]` attribute lets a `mod`
declaration point anywhere on disk regardless of where it's *nested* in the
module tree, which is what makes this work. In `routes/mcp.rs`:

```rust
#[path = "<name>/mcp.rs"]
mod name;
```

Second wrinkle: `#[tool_router]` only collects `#[tool]` methods from the one
`impl` block it's attached to, so pulling a tool out into its own file means
its router has to be combined with the rest by hand. Give the split-out
block a named router (`router = <name>_tool_router`) and a visibility wide
enough for the parent to call it (`vis = "pub(super)"` on the router, and the
tool method itself needs `pub(super) fn` too, since sibling test modules
under `routes::mcp` — e.g. `mcp_tests.rs` — need to reach it and a private
`fn` is only visible to *this* module's descendants):

```rust
//! MCP `<name>` tool — mirrors `GET /<name>`, split into its own `#[tool_router]`
//! block so it can be combined with the rest of `mcp.rs`'s router.

use rmcp::{model::CallToolResult, tool, tool_router};

use super::{ok, MoadimMcp};
use crate::routes::<name>::logic;

#[tool_router(router = <name>_tool_router, vis = "pub(super)")]
impl MoadimMcp {
    #[tool(description = "...")]
    pub(super) fn <name>(&self) -> Result<CallToolResult, rmcp::ErrorData> {
        Ok(ok(logic::build(/* ... */)))
    }
}

#[cfg(test)]
#[path = "mcp_tests.rs"]
mod mcp_tests;
```

Then in `routes/mcp.rs`, combine the routers in the `ServerHandler` impl —
**wrap the sum in parens**, since the macro splices the router expression
verbatim into things like `#router.call(tcc)`, and without parens `+`'s
precedence quietly binds to the wrong side:

```rust
#[tool_router] // no `server_handler` flag here — the combined impl below replaces it
impl MoadimMcp {
    // ...every other tool...
}

#[tool_handler(router = (Self::tool_router() + Self::<name>_tool_router()))]
impl rmcp::ServerHandler for MoadimMcp {}
```

If there's only ever one such split-out tool, this combined-router impl block
is a one-time thing — adding a *second* split-out tool just means extending
the same `+` chain, not writing a new `ServerHandler` impl.

## `mod.rs`

Thin wiring, plus any hidden-type re-exports utoipa needs (see the `health`
one for why `__path_<name>` has to be re-exported by name — it's not just
`pub use http::*`):

```rust
//! <Name>: shared logic (`logic.rs`), HTTP handler (`http.rs`), and MCP tool (`mcp.rs`,
//! declared from `routes::mcp` so it can reach `MoadimMcp`'s private state).

pub(crate) mod logic;
pub use logic::NameResponse;

#[path = "http.rs"]
mod http;
#[allow(
    unused_imports,
    reason = "utoipa's OpenApi derive resolves this hidden __path_<name> type via crate::routes::<name>::__path_<name>, generated by #[utoipa::path] on the re-exported handler below"
)]
pub use http::{__path_<name>, <name>};
```

Finally, register the new top-level module in `routes/mod.rs`:

```rust
pub mod <name>;
```
