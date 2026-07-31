//! CLI adapter for the routine move route.
//!
//! The command-line process remains a client of the running daemon: it resolves friendly
//! identifiers, serializes CLI flags into the move request body, and calls the daemon's REST
//! route instead of moving files directly.

use serde_json::{Map, Value};

use super::{routine_path, to_body};

/// Move a routine directory through the explicit `POST /routines/{id}/move` API.
pub(super) fn move_routine(routine: &str, folder: Option<String>, slug: Option<String>) -> i32 {
    let (id, current_slug) = match resolve_routine(routine) {
        Ok(found) => found,
        Err(code) => return code,
    };
    let Some(slug) = slug.or(current_slug) else {
        return 1;
    };
    let mut map = Map::new();
    map.insert("slug".to_string(), Value::String(slug));
    if let Some(folder) = folder {
        map.insert("folder".to_string(), Value::String(folder));
    }
    match crate::cli::http_request_json(
        "POST",
        &format!("{}/move", routine_path(&id)),
        Some(&to_body(map)),
    ) {
        Ok((status, resp)) if (200..300).contains(&status) => {
            print_move_result(&resp);
            0
        }
        Ok((status, resp)) => report_http_error(status, &resp),
        Err(_) => not_running(),
    }
}

/// Resolve an id/slug/relative path to the routine id and current slug.
fn resolve_routine(routine: &str) -> Result<(String, Option<String>), i32> {
    if let Ok((status, resp)) = crate::cli::http_request_json("GET", &routine_path(routine), None) {
        if (200..300).contains(&status) {
            return Ok(parse_routine_identity(&resp).unwrap_or_else(|| (routine.to_string(), None)));
        }
    } else {
        return Err(not_running());
    }
    let resp = match crate::cli::http_request_json("GET", "/api/v1/routines", None) {
        Ok((status, resp)) if (200..300).contains(&status) => resp,
        Ok((status, resp)) => {
            return Err(report_http_error(status, &resp));
        }
        Err(_) => return Err(not_running()),
    };
    let Ok(value) = serde_json::from_str::<Value>(&resp) else {
        eprintln!("error: could not parse routines list from server");
        return Err(1);
    };
    value
        .as_array()
        .and_then(|items| {
            items.iter().find_map(|item| {
                let id = item.get("id")?.as_str()?;
                let slug = item.get("slug").and_then(Value::as_str);
                let rel_path = item.get("rel_path").and_then(Value::as_str);
                (routine == id || Some(routine) == slug || Some(routine) == rel_path)
                    .then(|| (id.to_string(), slug.map(ToString::to_string)))
            })
        })
        .ok_or_else(|| {
            eprintln!("error: no routine with id, slug, or path {routine}");
            1
        })
}

/// Parse a single routine response into the id plus current filesystem slug.
fn parse_routine_identity(resp: &str) -> Option<(String, Option<String>)> {
    let value = serde_json::from_str::<Value>(resp).ok()?;
    let id = value.get("id")?.as_str()?.to_string();
    let slug = value
        .get("slug")
        .and_then(Value::as_str)
        .map(ToString::to_string);
    Some((id, slug))
}

/// Print a compact success line that exposes the filesystem-derived destination.
fn print_move_result(resp: &str) {
    let parsed = serde_json::from_str::<Value>(resp).ok();
    let Some(value) = parsed.as_ref() else {
        println!("{resp}");
        return;
    };
    let id = value.get("id").and_then(Value::as_str).unwrap_or("routine");
    let rel_path = value
        .get("rel_path")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    println!("moved routine {id} to {rel_path}");
}

/// Print a server-side error body and return the generic failure exit code.
fn report_http_error(status: u16, resp: &str) -> i32 {
    eprintln!("error: server returned HTTP {status}");
    if !resp.is_empty() {
        eprintln!("{resp}");
    }
    1
}

/// Print the standard liveness failure message and return the not-running exit code.
fn not_running() -> i32 {
    eprintln!("moadim is not running");
    crate::cli::EXIT_NOT_RUNNING
}

#[cfg(test)]
#[path = "cli_tests.rs"]
mod cli_tests;
