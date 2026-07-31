#![allow(
    clippy::wildcard_imports,
    clippy::too_many_arguments,
    clippy::needless_pass_by_value,
    dead_code,
    reason = "split-out module preserves existing code while keeping files under the linecheck limit"
)]
use super::*;

/// Route a parsed [`RoutineCmd`] to the matching `/routines` REST call.
pub(crate) fn dispatch_routine(cmd: RoutineCmd) -> i32 {
    match cmd {
        RoutineCmd::Create {
            schedule,
            title,
            agent,
            model,
            prompt,
            goal,
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
            model,
            prompt,
            goal,
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
            model,
            prompt,
            goal,
            repositories,
            machines,
            enabled,
            ttl_secs,
            max_runtime_secs,
            tags,
        } => {
            let mut map = Map::new();
            if !schedule.is_empty() {
                map.insert("schedules".to_string(), tags_value(schedule));
            }
            insert_opt(&mut map, "title", title.map(Value::String));
            insert_opt(&mut map, "agent", agent.map(Value::String));
            insert_opt(&mut map, "model", model.map(Value::String));
            insert_opt(&mut map, "prompt", prompt.map(Value::String));
            // Omitting `--goal` keeps the existing value (key absent); `--goal ""` clears it.
            insert_opt(&mut map, "goal", goal.map(Value::String));
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
            model,
            prompt,
            goal,
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
            model,
            prompt,
            goal,
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
        RoutineCmd::Move { id, folder, slug } => move_routine(&id, folder, slug),
        RoutineCmd::Trigger { id } => {
            request("POST", &format!("{}/trigger", routine_path(&id)), None)
        }
        RoutineCmd::Logs { id } => request("GET", &format!("{}/logs", routine_path(&id)), None),
        RoutineCmd::Ical => request("GET", "/api/v1/routines.ics", None),
    }
}

/// Build the `/api/v1/routines/{id}` path for a routine ID.
pub(crate) fn routine_path(id: &str) -> String {
    format!("/api/v1/routines/{id}")
}

pub(crate) fn routine_body(
    schedule: Vec<String>,
    title: String,
    agent: String,
    model: Option<String>,
    prompt: String,
    goal: Option<String>,
    repositories: Option<String>,
    machines: Option<String>,
    ttl_secs: Option<u64>,
    max_runtime_secs: Option<u64>,
    tags: Vec<String>,
    disabled: bool,
) -> Result<String, i32> {
    let mut map = Map::new();
    let primary = schedule.first().cloned().unwrap_or_default();
    map.insert("schedule".to_string(), Value::String(primary));
    map.insert("schedules".to_string(), tags_value(schedule));
    map.insert("title".to_string(), Value::String(title));
    map.insert("agent".to_string(), Value::String(agent));
    insert_opt(&mut map, "model", model.map(Value::String));
    map.insert("prompt".to_string(), Value::String(prompt));
    insert_opt(&mut map, "goal", goal.map(Value::String));
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
