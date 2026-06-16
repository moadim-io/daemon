//! Routine data model, agent registry, command builder, service functions, and HTTP handlers.
//!
//! A *routine* is a scheduled AI-agent task. Unlike a [`crate::cron_jobs::CronJob`] (which runs a
//! handler script), a routine launches an agent (claude code, codex, …) inside an interactive tmux
//! session rooted in a fresh workbench. moadim never clones the routine's `repositories`; it lists
//! them in the prompt as context and the agent clones any it needs.
//!
//! The module is split by concern:
//! - [`model`] — persisted types, API responses, and request bodies.
//! - [`agents`] — the agent registry and built-in default agent configs.
//! - [`defaults`] — built-in default routines seeded on startup when absent.
//! - [`command`] — prompt composition and the single-line launch command builder.
//! - [`service`] — store-mutating service functions (list/get/create/update/delete/trigger/logs).
//! - [`cleanup`] — auto-removal of finished, expired run workbenches (per-routine TTL).
//! - [`ical`] — iCalendar (`.ics`) export of upcoming routine fire times.
//! - [`handlers`] — the Axum HTTP handlers.

mod agents;
mod cleanup;
mod command;
mod defaults;
mod handlers;
mod ical;
mod model;
mod service;

pub use agents::*;
pub use cleanup::*;
pub use defaults::*;
pub use handlers::*;
pub use model::*;
pub use service::*;
// `command` holds only crate-internal helpers (slugify, compose_prompt, build_routine_command, …).
pub(crate) use command::*;

#[cfg(test)]
#[path = "mod_tests.rs"]
mod routines_tests;
