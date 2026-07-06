//! JSON request-body helpers and the loopback HTTP request/response cycle shared by every
//! data-plane subcommand in `commands.rs`, split out to stay under the line-count gate.

use serde_json::{Map, Value};

/// Convert a list of CLI `--tag` values into a JSON array of strings.
pub(crate) fn tags_value(tags: Vec<String>) -> Value {
    Value::Array(tags.into_iter().map(Value::String).collect())
}

/// Insert `key => value` into `map` only when `value` is `Some`, leaving the key absent otherwise so
/// PATCH bodies carry just the fields the user supplied.
pub(crate) fn insert_opt(map: &mut Map<String, Value>, key: &str, value: Option<Value>) {
    if let Some(value) = value {
        map.insert(key.to_string(), value);
    }
}

/// Parse an optional raw-JSON flag and insert it under `key` when present. Returns an exit code
/// (`2`) and prints a diagnostic when the supplied string is not valid JSON.
pub(crate) fn insert_json_opt(
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

/// Serialize a JSON object map into a compact request body string.
pub(crate) fn to_body(map: Map<String, Value>) -> String {
    Value::Object(map).to_string()
}

/// Send `method path` (with optional JSON `body`) to the running server, print the response, and map
/// it to a process exit code: `0` on a 2xx, `1` on any other HTTP status (the server's error body is
/// printed to stderr), and [`crate::cli::EXIT_NOT_RUNNING`] when no server is reachable.
pub(crate) fn request(method: &str, path: &str, body: Option<&str>) -> i32 {
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
pub(crate) fn print_body(body: &str) {
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
