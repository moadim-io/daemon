//! Data-plane CLI subcommands.
//!
//! These mirror the daemon's `/api/v1` REST routes (and the MCP tools) so most actions are
//! reachable from the command line too — routine flags and the global routine lock are
//! REST/MCP-only for now. Each subcommand is a thin client: it serializes its flags into the
//! same JSON the REST API expects, sends it to the running server over the loopback HTTP client in
//! [`crate::cli`], and prints the server's response. The daemon must already be running
//! (`moadim` / `moadim -i`); when it is not, these commands report that and exit
//! [`crate::cli::EXIT_NOT_RUNNING`].

use clap::{Parser, Subcommand};
use serde_json::{Map, Value};

#[allow(
    clippy::missing_docs_in_private_items,
    reason = "split-out module keeps the file under the linecheck limit"
)]
#[path = "commands_part2.rs"]
mod commands_part2;
pub(crate) use commands_part2::*;
#[allow(
    clippy::missing_docs_in_private_items,
    reason = "split-out module keeps the file under the linecheck limit"
)]
#[path = "commands_part3.rs"]
mod commands_part3;
pub(crate) use commands_part3::*;

/// Top-level parser for the data-plane subcommands, parsed from argv with the leading `moadim`
/// binary name already stripped (`no_binary_name`), so the first token is the subcommand keyword.
#[derive(Parser)]
#[command(
    name = "moadim",
    version,
    no_binary_name = true,
    about = "moadim data commands"
)]
struct DataCli {
    /// The selected data subcommand.
    #[command(subcommand)]
    command: DataCommand,
}

/// The data subcommand groups: routines and agents.
#[derive(Subcommand)]
pub(crate) enum DataCommand {
    /// Manage routines (create/list/get/update/replace/delete/trigger/logs/ical).
    ///
    /// Boxed because `RoutineCmd` (the largest variant by far now that the cron-job
    /// subcommand is gone) would otherwise blow up the size of every `DataCommand`
    /// value, including the trivial `Agents`/`Schedule` ones (`clippy::large_enum_variant`).
    #[command(subcommand, visible_alias = "routine")]
    Routines(Box<RoutineCmd>),
    /// Trigger a routine on its schedule by ID (invoked by the generated crontab line).
    #[command(subcommand, visible_alias = "sched")]
    Schedule(ScheduleCmd),
    /// Enable a routine (set `enabled = true`) by id or slug.
    Enable {
        /// Routine id or slug to enable.
        routine: String,
        /// Emit a machine-readable `{"routine","enabled"}` object instead of a status line.
        #[arg(long)]
        json: bool,
    },
    /// Disable a routine (set `enabled = false`) by id or slug.
    Disable {
        /// Routine id or slug to disable.
        routine: String,
        /// Emit a machine-readable `{"routine","enabled"}` object instead of a status line.
        #[arg(long)]
        json: bool,
    },
    /// List the available agent registry keys.
    Agents,
}

/// Schedule operations driven by the OS crontab, keyed only by ID.
#[derive(Subcommand)]
pub(crate) enum ScheduleCmd {
    /// Run a routine on its schedule by ID.
    ///
    /// This is what the generated crontab line invokes at each fire time. It records a *scheduled*
    /// trigger (not a manual one), so it maps to the routine's `scheduled-trigger` route rather than
    /// the manual `trigger` route.
    Trigger {
        /// UUID of the routine to trigger.
        id: String,
    },
}

/// Build the full create/replace JSON body for a routine, validating optional `repositories` as a
/// JSON array. Returns the serialized body, or an exit code (`2`) when `repositories` is invalid.
#[allow(
    clippy::too_many_arguments,
    reason = "all parameters map to distinct CLI flags with no natural grouping"
)]
#[path = "commands_http.rs"]
mod commands_http;
use commands_http::{insert_json_opt, insert_opt, request, tags_value, to_body};

#[path = "commands_routine_actions.rs"]
mod commands_routine_actions;
use commands_routine_actions::set_routine_enabled;

#[path = "routes/move_routine/cli.rs"]
mod move_routine_cli;
use move_routine_cli::move_routine;

#[cfg(test)]
#[path = "commands_tests.rs"]
mod commands_tests;
