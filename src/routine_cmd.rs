#![allow(
    clippy::wildcard_imports,
    clippy::too_many_arguments,
    clippy::needless_pass_by_value,
    dead_code,
    reason = "split-out module preserves existing code while keeping files under the linecheck limit"
)]
use super::*;

/// Routine operations, each mapping to a `/api/v1/routines` REST route.
#[derive(Subcommand)]
pub(crate) enum RoutineCmd {
    /// Create a new routine.
    Create {
        /// Cron expression (host local timezone, not UTC). Repeat to set multiple schedules.
        #[arg(long, required = true)]
        schedule: Vec<String>,
        /// Human-readable title.
        #[arg(long)]
        title: String,
        /// Agent registry key to launch.
        #[arg(long)]
        agent: String,
        /// Model ID to run the agent with (e.g. `claude-sonnet-4-6`); omit to use the agent's own
        /// default.
        #[arg(long)]
        model: Option<String>,
        /// Task prompt.
        #[arg(long)]
        prompt: String,
        /// Short (≤5 line) statement of the routine's goal — the "why" behind the prompt.
        #[arg(long)]
        goal: Option<String>,
        /// Repositories as a JSON array (e.g. `[{"repository":"url","branch":"main"}]`).
        #[arg(long)]
        repositories: Option<String>,
        /// Machines to run this routine on, as a JSON array (e.g. `["work","server"]`). Empty/omitted
        /// means the routine runs on no machine until assigned.
        #[arg(long)]
        machines: Option<String>,
        /// Workbench TTL in seconds for finished runs.
        #[arg(long)]
        ttl_secs: Option<u64>,
        /// Max runtime in seconds before the watchdog kills a run.
        #[arg(long)]
        max_runtime_secs: Option<u64>,
        /// Tag for the routine; repeat the flag to add several.
        #[arg(long = "tag")]
        tags: Vec<String>,
        /// Create the routine disabled instead of enabled (the default).
        #[arg(long)]
        disabled: bool,
    },
    /// List all routines.
    List,
    /// Get a single routine by ID.
    Get {
        /// UUID of the routine.
        id: String,
    },
    /// Update fields of an existing routine (only the flags you pass change).
    Update {
        /// UUID of the routine to update.
        id: String,
        /// New cron expression (host local timezone, not UTC). Repeat to replace all schedules.
        #[arg(long)]
        schedule: Vec<String>,
        /// New title.
        #[arg(long)]
        title: Option<String>,
        /// New agent registry key.
        #[arg(long)]
        agent: Option<String>,
        /// New model ID, or an empty string to clear the override back to the agent's own default.
        #[arg(long)]
        model: Option<String>,
        /// New prompt.
        #[arg(long)]
        prompt: Option<String>,
        /// New goal (≤5 lines), or an empty string to clear it. Omit to keep the existing value.
        #[arg(long)]
        goal: Option<String>,
        /// New repositories as a JSON array.
        #[arg(long)]
        repositories: Option<String>,
        /// New machines targeting list as a JSON array (e.g. `["work","server"]`).
        #[arg(long)]
        machines: Option<String>,
        /// New enabled state (`true`/`false`).
        #[arg(long)]
        enabled: Option<bool>,
        /// New workbench TTL in seconds.
        #[arg(long)]
        ttl_secs: Option<u64>,
        /// New max runtime in seconds.
        #[arg(long)]
        max_runtime_secs: Option<u64>,
        /// Replacement tag; repeat the flag to set several. Passing any `--tag` replaces the whole
        /// tag list; omit it to keep the existing tags.
        #[arg(long = "tag")]
        tags: Vec<String>,
    },
    /// Replace a routine wholesale (all fields, like create but for an existing ID).
    Replace {
        /// UUID of the routine to replace.
        id: String,
        /// Cron expression (host local timezone, not UTC). Repeat to set multiple schedules.
        #[arg(long, required = true)]
        schedule: Vec<String>,
        /// Human-readable title.
        #[arg(long)]
        title: String,
        /// Agent registry key to launch.
        #[arg(long)]
        agent: String,
        /// Model ID to run the agent with (e.g. `claude-sonnet-4-6`); omit to use the agent's own
        /// default.
        #[arg(long)]
        model: Option<String>,
        /// Task prompt.
        #[arg(long)]
        prompt: String,
        /// Short (≤5 line) statement of the routine's goal — the "why" behind the prompt.
        #[arg(long)]
        goal: Option<String>,
        /// Repositories as a JSON array.
        #[arg(long)]
        repositories: Option<String>,
        /// Machines to run this routine on, as a JSON array (e.g. `["work","server"]`).
        #[arg(long)]
        machines: Option<String>,
        /// Workbench TTL in seconds for finished runs.
        #[arg(long)]
        ttl_secs: Option<u64>,
        /// Max runtime in seconds before the watchdog kills a run.
        #[arg(long)]
        max_runtime_secs: Option<u64>,
        /// Tag for the routine; repeat the flag to add several.
        #[arg(long = "tag")]
        tags: Vec<String>,
        /// Replace into a disabled state instead of enabled (the default).
        #[arg(long)]
        disabled: bool,
    },
    /// Delete a routine by ID.
    Delete {
        /// UUID of the routine to delete.
        id: String,
    },
    /// Move a routine to another filesystem folder and/or slug.
    Move {
        /// Routine id, slug, or relative path to move.
        id: String,
        /// New parent folder relative to `routines/`; omit or pass blank for the root.
        #[arg(long)]
        folder: Option<String>,
        /// New routine directory name inside the folder. Omit to preserve the current slug.
        #[arg(long)]
        slug: Option<String>,
    },
    /// Manually trigger a routine outside its schedule.
    Trigger {
        /// UUID of the routine to trigger.
        id: String,
    },
    /// Print a routine's newest run log.
    Logs {
        /// UUID of the routine whose logs to print.
        id: String,
    },
    /// Print the iCalendar feed of upcoming routine fire times.
    Ical,
}
include!("run.rs");
