use wasm_bindgen::prelude::*;
use web_sys::console;

/// Initialize the WASM module. Call this once on startup.
#[wasm_bindgen]
pub fn wasm_init() {
    console_log::init_with_level(log::Level::Info).unwrap();
    web_sys::console::log_1(&"Moadim WASM module initialized".into());
}

/// Query health from the native server.
/// Returns a JSON string: {"status":"ok","uptime_secs":N,"running":true}
#[wasm_bindgen]
pub async fn wasm_query_health() -> Result<String, JsValue> {
    let resp = web_sys::RequestInit::new();
    resp.set_method("GET");

    let window = web_sys::window().ok_or("no window")?;
    let url = format!("{}/health", window.location().origin()?);
    let request = web_sys::Request::new_with_str_and_init(&url, &resp)?;

    let resp = window
        .fetch_with_request(&request)
        .await
        .map_err(|_| "fetch failed")?;

    let json = resp.json().map_err(|_| "failed to parse JSON")??;
    Ok(serde_wasm_bindgen::to_value(&json)
        .map_err(|e| e.to_string())?
        .as_string()
        .ok_or("not a string")?)
}

/// Echo a message to the server.
/// Returns a JSON string: {"message":"...","timestamp":N}
#[wasm_bindgen]
pub async fn wasm_echo(message: &str) -> Result<String, JsValue> {
    let body = serde_json::json!({"message": message}).to_string();
    let resp = web_sys::RequestInit::new();
    resp.set_method("POST");
    resp.set_body(&body);

    let window = web_sys::window().ok_or("no window")?;
    let url = format!("{}/echo", window.location().origin()?);
    let request = web_sys::Request::new_with_str_and_init(&url, &resp)?;

    let resp = window
        .fetch_with_request(&request)
        .await
        .map_err(|_| "fetch failed")?;

    let json = resp.json().map_err(|_| "failed to parse JSON")??;
    Ok(serde_wasm_bindgen::to_value(&json)
        .map_err(|e| e.to_string())?
        .as_string()
        .ok_or("not a string")?)
}

/// Get server info.
/// Returns a JSON string: {"name":"moadim-server","version":"0.1.0",...}
#[wasm_bindgen]
pub async fn wasm_get_info() -> Result<String, JsValue> {
    let resp = web_sys::RequestInit::new();
    resp.set_method("GET");

    let window = web_sys::window().ok_or("no window")?;
    let url = format!("{}/info", window.location().origin()?);
    let request = web_sys::Request::new_with_str_and_init(&url, &resp)?;

    let resp = window
        .fetch_with_request(&request)
        .await
        .map_err(|_| "fetch failed")?;

    let json = resp.json().map_err(|_| "failed to parse JSON")??;
    Ok(serde_wasm_bindgen::to_value(&json)
        .map_err(|e| e.to_string())?
        .as_string()
        .ok_or("not a string")?)
}

/// Return the WASM mode string.
#[wasm_bindgen]
pub fn wasm_mode() -> &'static str {
    "wasm"
}

/// Simple checksum utility.
#[wasm_bindgen]
pub fn wasm_checksum(input: &str) -> String {
    let mut hash: u32 = 5381;
    for byte in input.bytes() {
        hash = hash.wrapping_mul(33).wrapping_add(byte as u32);
    }
    format!("{:08x}", hash)
}

/// Reverse a string.
#[wasm_bindgen]
pub fn wasm_reverse(input: &str) -> String {
    input.chars().rev().collect()
}

/// Uppercase a string.
#[wasm_bindgen]
pub fn wasm_uppercase(input: &str) -> String {
    input.to_uppercase()
}
