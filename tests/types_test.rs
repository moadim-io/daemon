use server::config::Config;
use server::types::{ConfigResponse, HealthResponse, InfoResponse};

#[test]
fn config_default_host() {
    let c = Config::new();
    assert_eq!(c.bind_host, "127.0.0.1");
}

#[test]
fn config_default_port() {
    let c = Config::new();
    assert_eq!(c.bind_port, 8080);
}

#[test]
fn health_response_roundtrip() {
    let resp = HealthResponse {
        status: "ok",
        uptime_secs: 12345,
        running: true,
    };
    let json = serde_json::to_string(&resp).unwrap();
    let parsed: HealthResponse = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed.status, "ok");
    assert_eq!(parsed.uptime_secs, 12345);
    assert!(parsed.running);
}

#[test]
fn info_response_serializes() {
    let resp = InfoResponse {
        name: "test".into(),
        version: "0.1.0".into(),
        host: "localhost".into(),
        uptime_secs: 0,
        features: vec!["test".into()],
    };
    let json = serde_json::to_string(&resp).unwrap();
    assert!(json.contains("\"name\":\"test\""));
}

#[test]
fn config_response_roundtrip() {
    let resp = ConfigResponse {
        bind_host: "0.0.0.0".into(),
        bind_port: 3000,
        log_level: "debug".into(),
        wasm_enabled: false,
    };
    let json = serde_json::to_string(&resp).unwrap();
    let parsed: ConfigResponse = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed.bind_port, 3000);
    assert!(!parsed.wasm_enabled);
}
