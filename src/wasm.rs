use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn wasm_start() {
    console_log::init_with_level(log::Level::Info).unwrap();
    web_sys::console::log_1(&"WASM server module loaded".into());
}

#[wasm_bindgen]
pub fn wasm_health() -> String {
    serde_json::json!({
        "status": "ok",
        "mode": "wasm",
    })
    .to_string()
}
