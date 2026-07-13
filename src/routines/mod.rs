//! Routine data model, agent registry, command builder, service functions, and HTTP handlers.
//!
//! A *routine* is a scheduled AI-agent task: it launches an agent (claude code, codex, …) inside an
//! interactive tmux session rooted in a fresh workbench. Before each run, when a repository's
//! routine has `auto_pull` enabled (the default), moadim fetches and fast-forward pulls it into a
//! persistent per-routine cache (see `repo_sync`, #1132) — best-effort, raising an `auto_pull_failed`
//! flag rather than blocking the run on failure. The agent's own workbench still starts empty; it
//! lists the repositories in the prompt as context and clones any it needs there itself.
//!
//! The module is split by concern:
//! - `model` — persisted types, API responses, and request bodies.
//! - `agents` — the agent registry and built-in default agent configs.
//! - `defaults` — built-in default routines seeded on startup when absent.
//! - `command` — prompt composition and the single-line launch command builder.
//! - `service` — store-mutating service functions (list/get/create/update/delete/trigger/logs).
//! - `cleanup` — auto-removal of finished, expired run workbenches (per-routine TTL).
//! - `ical` — iCalendar (`.ics`) export of upcoming routine fire times.
//! - `flags` — agent-raised gap/bug/edge-case notes attached to a routine.
//! - `repo_sync` — auto-pull: fetch + fast-forward each repository into a persistent cache.
//! - `handlers` — the Axum HTTP handlers.

mod agents;
mod cleanup;
mod command;
mod concurrency_cap;
mod defaults;
pub mod flags;
mod handlers;
mod ical;
mod model;
mod repo_sync;
mod run_history;
mod service;

pub use agents::*;
pub use cleanup::*;
pub use defaults::*;
pub use flags::{Flag, FlagScope};
pub use handlers::*;
pub use model::*;
pub use service::*;
// `command` holds only crate-internal helpers (slugify, compose_prompt, build_routine_command, …).
pub(crate) use command::*;
// `concurrency_cap` is a crate-internal config knob for `service_trigger::spawn_routine_command`.
pub(crate) use concurrency_cap::{max_concurrent_runs, MAX_CONCURRENT_RUNS_ENV};
// `repo_sync` is invoked only from `service_trigger::spawn_routine_command`.
pub(crate) use repo_sync::sync_repositories;

#[cfg(test)]
#[path = "mod_tests.rs"]
mod routines_tests;

#[cfg(test)]
#[path = "mod_agents_tests.rs"]
mod mod_agents_tests;

#[cfg(test)]
#[path = "mod_agents_reload_tests.rs"]
mod mod_agents_reload_tests;
