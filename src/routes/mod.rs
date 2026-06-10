//! Network-facing layer. Each submodule owns one protocol transport.
//!
//! Business logic lives in [`crate::cron_jobs`]. Modules here translate
//! between protocol representations and the service-layer functions.

pub mod graphql;
pub mod http;
pub mod mcp;
