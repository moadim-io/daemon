//! Data-plane CLI subcommands.
//!
//! These mirror the daemon's `/api/v1` REST routes (and the MCP tools) so every action is reachable
//! from the command line too. Each subcommand is a thin client: it serializes its flags into the
//! same JSON the REST API expects, sends it to the running server over the loopback HTTP client in
//! [`crate::cli`], and prints the server's response. The daemon must already be running
//! (`moadim` / `moadim -i`); when it is not, these commands report that and exit
//! [`crate::cli::EXIT_NOT_RUNNING`].

use clap::{Parser, Subcommand};
use serde_json::{Map, Value};

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

/// The data subcommand groups: routines, agents, and echo.
#[derive(Subcommand)]
enum DataCommand {
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
    /// List the available agent registry keys.
    Agents,
    /// Echo a message back via the server, with a server timestamp.
    Echo {
        /// The message to echo.
        message: String,
    },
}

/// Schedule operations driven by the OS crontab, keyed only by ID.
#[derive(Subcommand)]
enum ScheduleCmd {
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

/// Routine operations, each mapping to a `/api/v1/routines` REST route.
#[derive(Subcommand)]
enum RoutineCmd {
    /// Create a new routine.
    Create {
        /// Cron expression (host local timezone, not UTC).
        #[arg(long)]
        schedule: String,
        /// Human-readable title.
        #[arg(long)]
        title: String,
        /// Agent registry key to launch.
        #[arg(long)]
        agent: String,
        /// Task prompt.
        #[arg(long)]
        prompt: String,
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
        /// New cron expression (host local timezone, not UTC).
        #[arg(long)]
        schedule: Option<String>,
        /// New title.
        #[arg(long)]
        title: Option<String>,
        /// New agent registry key.
        #[arg(long)]
        agent: Option<String>,
        /// New prompt.
        #[arg(long)]
        prompt: Option<String>,
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
        /// Cron expression (host local timezone, not UTC).
        #[arg(long)]
        schedule: String,
        /// Human-readable title.
        #[arg(long)]
        title: String,
        /// Agent registry key to launch.
        #[arg(long)]
        agent: String,
        /// Task prompt.
        #[arg(long)]
        prompt: String,
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

/// Parse `args` (argv with the binary name stripped) and run the selected data subcommand against
/// the running server, returning the process exit code to surface.
///
/// On a clap parse error (bad flags, `--help`, `--version`) the formatted message is printed and the
/// matching code returned (`0` for help/version, `2` for a usage error), mirroring clap conventions
/// without aborting the process so the path stays unit-testable.
pub fn run(args: Vec<String>) -> i32 {
    match DataCli::try_parse_from(args) {
        Ok(cli) => dispatch(cli.command),
        Err(err) => {
            let _ = err.print();
            match err.kind() {
                clap::error::ErrorKind::DisplayHelp | clap::error::ErrorKind::DisplayVersion => 0,
                _ => 2,
            }
        }
    }
}

/// Route a parsed [`DataCommand`] to the matching REST call.
fn dispatch(command: DataCommand) -> i32 {
    match command {
        DataCommand::Routines(cmd) => dispatch_routine(*cmd),
        DataCommand::Schedule(ScheduleCmd::Trigger { id }) => request(
            "POST",
            &format!("{}/scheduled-trigger", routine_path(&id)),
            None,
        ),
        DataCommand::Agents => request("GET", "/api/v1/agents", None),
        DataCommand::Echo { message } => {
            let body = object([("message", Value::String(message))]);
            request("POST", "/api/v1/echo", Some(&body))
        }
    }
}

/// Route a parsed [`RoutineCmd`] to the matching `/routines` REST call.
fn dispatch_routine(cmd: RoutineCmd) -> i32 {
    match cmd {
        RoutineCmd::Create {
            schedule,
            title,
            agent,
            prompt,
            repositories,
            machines,
            ttl_secs,
            max_runtime_secs,
            tags,
            disabled,
        } => match routine_body(
            schedule,
            title,
            agent,
            prompt,
            repositories,
            machines,
            ttl_secs,
            max_runtime_secs,
            tags,
            disabled,
        ) {
            Ok(body) => request("POST", "/api/v1/routines", Some(&body)),
            Err(code) => code,
        },
        RoutineCmd::List => request("GET", "/api/v1/routines", None),
        RoutineCmd::Get { id } => request("GET", &routine_path(&id), None),
        RoutineCmd::Update {
            id,
            schedule,
            title,
            agent,
            prompt,
            repositories,
            machines,
            enabled,
            ttl_secs,
            max_runtime_secs,
            tags,
        } => {
            let mut map = Map::new();
            insert_opt(&mut map, "schedule", schedule.map(Value::String));
            insert_opt(&mut map, "title", title.map(Value::String));
            insert_opt(&mut map, "agent", agent.map(Value::String));
            insert_opt(&mut map, "prompt", prompt.map(Value::String));
            match insert_json_opt(&mut map, "repositories", repositories) {
                Ok(()) => {}
                Err(code) => return code,
            }
            match insert_json_opt(&mut map, "machines", machines) {
                Ok(()) => {}
                Err(code) => return code,
            }
            insert_opt(&mut map, "enabled", enabled.map(Value::Bool));
            insert_opt(&mut map, "ttl_secs", ttl_secs.map(Value::from));
            insert_opt(
                &mut map,
                "max_runtime_secs",
                max_runtime_secs.map(Value::from),
            );
            // Any `--tag` replaces the whole list; no `--tag` leaves tags untouched (key absent).
            insert_opt(
                &mut map,
                "tags",
                (!tags.is_empty()).then(|| tags_value(tags)),
            );
            request("PATCH", &routine_path(&id), Some(&to_body(map)))
        }
        RoutineCmd::Replace {
            id,
            schedule,
            title,
            agent,
            prompt,
            repositories,
            machines,
            ttl_secs,
            max_runtime_secs,
            tags,
            disabled,
        } => match routine_body(
            schedule,
            title,
            agent,
            prompt,
            repositories,
            machines,
            ttl_secs,
            max_runtime_secs,
            tags,
            disabled,
        ) {
            Ok(body) => request("PUT", &routine_path(&id), Some(&body)),
            Err(code) => code,
        },
        RoutineCmd::Delete { id } => request("DELETE", &routine_path(&id), None),
        RoutineCmd::Trigger { id } => {
            request("POST", &format!("{}/trigger", routine_path(&id)), None)
        }
        RoutineCmd::Logs { id } => request("GET", &format!("{}/logs", routine_path(&id)), None),
        RoutineCmd::Ical => request("GET", "/api/v1/routines.ics", None),
    }
}

/// Build the `/api/v1/routines/{id}` path for a routine ID.
fn routine_path(id: &str) -> String {
    format!("/api/v1/routines/{id}")
}

/// Build the full create/replace JSON body for a routine, validating optional `repositories` as a
/// JSON array. Returns the serialized body, or an exit code (`2`) when `repositories` is invalid.
#[allow(clippy::too_many_arguments)]
fn routine_body(
    schedule: String,
    title: String,
    agent: String,
    prompt: String,
    repositories: Option<String>,
    machines: Option<String>,
    ttl_secs: Option<u64>,
    max_runtime_secs: Option<u64>,
    tags: Vec<String>,
    disabled: bool,
) -> Result<String, i32> {
    let mut map = Map::new();
    map.insert("schedule".to_string(), Value::String(schedule));
    map.insert("title".to_string(), Value::String(title));
    map.insert("agent".to_string(), Value::String(agent));
    map.insert("prompt".to_string(), Value::String(prompt));
    insert_json_opt(&mut map, "repositories", repositories)?;
    insert_json_opt(&mut map, "machines", machines)?;
    insert_opt(&mut map, "ttl_secs", ttl_secs.map(Value::from));
    insert_opt(
        &mut map,
        "max_runtime_secs",
        max_runtime_secs.map(Value::from),
    );
    map.insert("tags".to_string(), tags_value(tags));
    map.insert("enabled".to_string(), Value::Bool(!disabled));
    Ok(to_body(map))
}

/// Convert a list of CLI `--tag` values into a JSON array of strings.
fn tags_value(tags: Vec<String>) -> Value {
    Value::Array(tags.into_iter().map(Value::String).collect())
}

/// Insert `key => value` into `map` only when `value` is `Some`, leaving the key absent otherwise so
/// PATCH bodies carry just the fields the user supplied.
fn insert_opt(map: &mut Map<String, Value>, key: &str, value: Option<Value>) {
    if let Some(value) = value {
        map.insert(key.to_string(), value);
    }
}

/// Parse an optional raw-JSON flag and insert it under `key` when present. Returns an exit code
/// (`2`) and prints a diagnostic when the supplied string is not valid JSON.
fn insert_json_opt(
    map: &mut Map<String, Value>,
    key: &str,
    raw: Option<String>,
) -> Result<(), i32> {
    let Some(raw) = raw else { return Ok(()) };
    match serde_json::from_str::<Value>(&raw) {
        Ok(value) => {
            map.insert(key.to_string(), value);
            Ok(())
        }
        Err(err) => {
            eprintln!("error: --{key} is not valid JSON: {err}");
            Err(2)
        }
    }
}

/// Build a small JSON object body from key/value pairs.
fn object<const N: usize>(pairs: [(&str, Value); N]) -> String {
    let mut map = Map::new();
    for (key, value) in pairs {
        map.insert(key.to_string(), value);
    }
    to_body(map)
}

/// Serialize a JSON object map into a compact request body string.
fn to_body(map: Map<String, Value>) -> String {
    Value::Object(map).to_string()
}

/// Send `method path` (with optional JSON `body`) to the running server, print the response, and map
/// it to a process exit code: `0` on a 2xx, `1` on any other HTTP status (the server's error body is
/// printed to stderr), and [`crate::cli::EXIT_NOT_RUNNING`] when no server is reachable.
fn request(method: &str, path: &str, body: Option<&str>) -> i32 {
    match crate::cli::http_request_json(method, path, body) {
        Ok((status, resp)) if (200..300).contains(&status) => {
            print_body(&resp);
            0
        }
        Ok((status, resp)) => {
            eprintln!("error: server returned HTTP {status}");
            if !resp.is_empty() {
                eprintln!("{resp}");
            }
            1
        }
        Err(_) => {
            eprintln!("moadim is not running");
            crate::cli::EXIT_NOT_RUNNING
        }
    }
}

/// Print a successful response body, pretty-printing it when it parses as JSON and echoing it raw
/// (e.g. plain-text logs / iCalendar feeds) otherwise.
fn print_body(body: &str) {
    if body.is_empty() {
        return;
    }
    match serde_json::from_str::<Value>(body) {
        Ok(value) => println!(
            "{}",
            serde_json::to_string_pretty(&value).unwrap_or_default()
        ),
        Err(_) => println!("{body}"),
    }
}

#[cfg(test)]
#[path = "commands_tests.rs"]
mod commands_tests;
