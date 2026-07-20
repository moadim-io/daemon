//! Network-facing layer. Each submodule owns one protocol transport.
//!
//! Business logic lives in [`crate::routines`]. Modules here translate
//! between protocol representations and the service-layer functions.

pub mod cleanup_workbenches;
pub mod create_flag;
pub mod create_routine;
pub mod delete_routine;
pub mod get_lock_status;
pub mod get_routine;
pub mod health;
pub mod http;
pub mod list_agents;
pub mod list_flags;
pub mod list_routine_runs;
pub mod list_routines;
pub mod mcp;
pub mod metrics;
pub mod resolve_flag;
pub mod restart;
pub mod shutdown;
pub mod trigger_routine;
pub mod update_routine;
