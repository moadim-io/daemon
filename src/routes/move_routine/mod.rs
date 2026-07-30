//! Move routine: shared logic (`logic.rs`), HTTP handler (`http.rs`), and MCP tool (`mcp.rs`).

pub(crate) mod logic;

#[path = "http.rs"]
mod http;
#[allow(
    unused_imports,
    reason = "utoipa's OpenApi derive resolves this hidden __path_move_routine type via crate::routes::move_routine::__path_move_routine"
)]
pub use http::{__path_move_routine, move_routine};
