#![allow(clippy::missing_docs_in_private_items)]

use super::*;
use crate::cron_jobs::CronJob;

fn test_job(id: &str) -> CronJob {
    CronJob {
        id: id.to_string(),
        schedule: "@daily".to_string(),
        handler: "test-handler".to_string(),
        metadata: serde_json::json!({"key": "val"}),
        enabled: true,
        source: "managed".to_string(),
        created_at: 1000,
        updated_at: 2000,
        last_triggered_at: Some(3000),
    }
}

#[test]
fn metadata_roundtrip() {
    let val = serde_json::json!({"key": "value", "num": 42});
    let table = json_to_toml_table(&val);
    let back = metadata_to_json(&table);
    assert_eq!(back["key"], "value");
    assert_eq!(back["num"], 42);
}

#[test]
fn metadata_roundtrip_empty_object() {
    let val = serde_json::json!({});
    let table = json_to_toml_table(&val);
    let back = metadata_to_json(&table);
    assert!(back.as_object().unwrap().is_empty());
}

#[test]
fn json_to_toml_table_non_object_returns_empty() {
    let val = serde_json::json!([1, 2, 3]);
    let table = json_to_toml_table(&val);
    assert!(table.is_empty());
}

#[test]
fn json_to_toml_table_null_returns_empty() {
    let table = json_to_toml_table(&serde_json::Value::Null);
    assert!(table.is_empty());
}

#[test]
fn read_job_toml_missing_returns_none() {
    let path = std::path::PathBuf::from("/nonexistent/path/job.toml");
    assert!(read_job_toml(&path).is_none());
}

#[test]
fn write_and_load_roundtrip() {
    let id = "test-write-load-roundtrip";
    let job = test_job(id);

    write_job(&job).expect("write_job failed");

    let loaded = load_job_from_dir(id).expect("load_job_from_dir failed");
    assert_eq!(loaded.id, job.id);
    assert_eq!(loaded.schedule, job.schedule);
    assert_eq!(loaded.handler, job.handler);
    assert_eq!(loaded.enabled, job.enabled);
    assert_eq!(loaded.created_at, job.created_at);
    assert_eq!(loaded.updated_at, job.updated_at);
    assert_eq!(loaded.last_triggered_at, job.last_triggered_at);
    assert_eq!(loaded.metadata["key"], "val");

    remove_job_dir(id).expect("cleanup failed");
}

#[test]
fn remove_job_dir_nonexistent_is_ok() {
    assert!(remove_job_dir("test-nonexistent-9999999").is_ok());
}

#[test]
fn remove_job_dir_removes_directory() {
    let id = "test-remove-dir";
    let job = test_job(id);
    write_job(&job).expect("write_job failed");

    let dir = crate::paths::job_dir(id);
    assert!(dir.exists());

    remove_job_dir(id).expect("remove failed");
    assert!(!dir.exists());
}

#[test]
fn load_store_returns_written_job() {
    let id = "test-load-store-job";
    let job = test_job(id);
    write_job(&job).expect("write_job failed");

    let store = load_store();
    let loaded = store.lock().unwrap().get(id).cloned();
    assert!(loaded.is_some(), "job not found in loaded store");
    assert_eq!(loaded.unwrap().handler, "test-handler");

    remove_job_dir(id).expect("cleanup failed");
}

#[test]
fn write_job_creates_gitignore() {
    let id = "test-gitignore-creation";
    let job = test_job(id);
    write_job(&job).expect("write_job failed");

    let gitignore = crate::paths::job_gitignore_path(id);
    assert!(gitignore.exists());
    let content = std::fs::read_to_string(&gitignore).unwrap();
    assert!(content.contains("*.local.*"));

    remove_job_dir(id).expect("cleanup failed");
}

#[test]
fn local_toml_overrides_base_handler() {
    let id = "test-local-override-handler";
    let job = test_job(id);
    write_job(&job).expect("write_job failed");

    let local_path = crate::paths::job_local_toml_path(id);
    std::fs::write(
        &local_path,
        "handler = \"overridden\"\n\n[metadata]\nlocal_key = \"local_value\"\n",
    )
    .unwrap();

    let loaded = load_job_from_dir(id).expect("load failed");
    assert_eq!(loaded.handler, "overridden");
    assert_eq!(loaded.metadata["local_key"], "local_value");

    remove_job_dir(id).expect("cleanup failed");
}

#[test]
fn write_job_twice_does_not_fail_on_existing_gitignore() {
    let id = "test-write-idempotent";
    let job = test_job(id);
    write_job(&job).expect("first write");
    write_job(&job).expect("second write (gitignore already exists)");
    remove_job_dir(id).expect("cleanup failed");
}

#[test]
fn load_store_skips_non_directory_entries() {
    let jobs_dir = crate::paths::jobs_dir();
    std::fs::create_dir_all(&jobs_dir).unwrap();
    let fake = jobs_dir.join("not-a-job.txt");
    std::fs::write(&fake, "hello").unwrap();

    let store = load_store();
    let _ = store; // must not panic

    std::fs::remove_file(&fake).unwrap();
}

#[test]
fn load_store_from_dir_missing_dir_returns_empty_store() {
    let store = load_store_from_dir(std::path::Path::new("/nonexistent-jobs-dir-99999"));
    assert!(store.lock().unwrap().is_empty());
}
