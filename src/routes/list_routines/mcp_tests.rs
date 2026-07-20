#![allow(
    clippy::missing_docs_in_private_items,
    reason = "test helpers and fixtures do not need doc comments"
)]

use super::MoadimMcp;
use crate::routes::mcp::mcp_types::ListRoutinesParam;

fn make_handler() -> MoadimMcp {
    MoadimMcp::new(
        crate::routines::new_store(),
        crate::paths::routines_dir(),
        0,
        std::sync::Arc::new(tokio::sync::Notify::new()),
    )
}

#[test]
fn list_routines_empty() {
    use rmcp::handler::server::wrapper::Parameters;
    let handler = make_handler();
    let result = handler
        .list_routines(Parameters(ListRoutinesParam {
            local_only: None,
            include_prompts: None,
        }))
        .unwrap();
    assert!(!result.is_error.unwrap_or(false));
}
