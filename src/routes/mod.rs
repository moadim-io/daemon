//! Network-facing layer. Each submodule owns one protocol transport.
//!
//! Business logic lives in [`crate::routines`]. Modules here translate
//! between protocol representations and the service-layer functions.

pub mod get_lock_status;
pub mod health;
pub mod http;
pub mod mcp;
pub mod restart;
pub mod shutdown;
