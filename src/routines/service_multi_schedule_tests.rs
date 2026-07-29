#![allow(
    clippy::missing_docs_in_private_items,
    reason = "test helpers and fixtures do not need doc comments"
)]

use super::*;
use crate::routines::new_store;
use std::sync::Mutex;

struct TempHome(std::path::PathBuf);
impl TempHome {
    fn set() -> Self {
        let dir = std::env::temp_dir().join(format!("moadim-multisched-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).expect("create temp home");
        // SAFETY: tests in this crate run single-threaded.
        unsafe { std::env::set_var("MOADIM_HOME_OVERRIDE", &dir) };
        Self(dir)
    }
}
impl Drop for TempHome {
    fn drop(&mut self) {
        // SAFETY: tests in this crate run single-threaded.
        unsafe { std::env::remove_var("MOADIM_HOME_OVERRIDE") };
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

static PATH_GUARD: Mutex<()> = Mutex::new(());

fn with_working_crontab(body: impl FnOnce()) {
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt as _;

    let guard = PATH_GUARD
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let base = std::env::temp_dir().join(format!("moadim-multicron-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&base).unwrap();
    let script = base.join("crontab-ok.sh");
    std::fs::write(
        &script,
        "#!/bin/sh\nif [ \"$1\" = \"-\" ]; then cat > /dev/null; fi\nexit 0\n",
    )
    .unwrap();
    #[cfg(unix)]
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();

    let saved = std::env::var_os("MOADIM_CRONTAB_BIN");
    // SAFETY: tests in this crate run single-threaded.
    unsafe { std::env::set_var("MOADIM_CRONTAB_BIN", &script) };
    body();
    // SAFETY: tests in this crate run single-threaded.
    unsafe {
        match saved {
            Some(value) => std::env::set_var("MOADIM_CRONTAB_BIN", value),
            None => std::env::remove_var("MOADIM_CRONTAB_BIN"),
        }
    }
    let _ = std::fs::remove_dir_all(&base);
    drop(guard);
}

fn valid_create_request() -> CreateRoutineRequest {
    CreateRoutineRequest {
        model: None,
        goal: None,
        schedule: "@daily".into(),
        schedules: vec![],
        title: "Valid Title".into(),
        agent: "claude".into(),
        prompt: "do the thing".into(),
        repositories: vec![],
        machines: vec![crate::machine::current_machine()],
        enabled: true,
        ttl_secs: None,
        max_runtime_secs: None,
        tags: vec![],
        env: std::collections::HashMap::new(),
        failure_threshold: None,
    }
}

fn empty_update_request() -> UpdateRoutineRequest {
    UpdateRoutineRequest {
        model: None,
        goal: None,
        schedule: None,
        schedules: None,
        title: None,
        agent: None,
        prompt: None,
        repositories: None,
        machines: None,
        enabled: None,
        ttl_secs: None,
        max_runtime_secs: None,
        tags: None,
        env: None,
        failure_threshold: None,
    }
}

#[test]
fn validate_schedules_rejects_an_empty_list() {
    assert!(matches!(
        validate_and_normalize_schedules(&[]),
        Err(AppError::BadRequest(message)) if message.contains("at least one schedule")
    ));
}

#[test]
fn svc_create_accepts_multiple_schedules_and_persists_sidecar_lines() {
    let _home = TempHome::set();
    with_working_crontab(|| {
        let store = new_store();
        let resp = svc_create(
            &store,
            CreateRoutineRequest {
                schedule: "@daily".into(),
                schedules: vec!["@hourly".into(), "0 9 * * *".into()],
                title: "Multi Schedule Create".into(),
                ..valid_create_request()
            },
        )
        .expect("create multi-schedule routine");
        assert_eq!(resp.routine.schedule, "@hourly");
        assert_eq!(resp.routine.schedules, vec!["@hourly", "0 9 * * *"]);
        let path = crate::paths::routine_cron_path(&slugify("Multi Schedule Create"));
        assert_eq!(
            std::fs::read_to_string(path).unwrap(),
            "@hourly\n0 9 * * *\n"
        );
    });
}

#[test]
fn svc_update_rejects_schedule_and_schedules_together() {
    let _home = TempHome::set();
    let store = new_store();
    let mut req = empty_update_request();
    req.schedule = Some("@daily".into());
    req.schedules = Some(vec!["@hourly".into()]);
    assert!(matches!(
        svc_update(&store, "missing", req),
        Err(AppError::BadRequest(message)) if message.contains("either schedule or schedules")
    ));
}

#[test]
fn svc_update_replaces_multiple_schedules() {
    let _home = TempHome::set();
    with_working_crontab(|| {
        let store = new_store();
        let created = svc_create(&store, valid_create_request()).expect("create routine");
        let mut req = empty_update_request();
        req.schedules = Some(vec!["@hourly".into(), "0 9 * * *".into()]);
        let resp = svc_update(&store, &created.routine.id, req).expect("update schedules");
        assert_eq!(resp.routine.schedule, "@hourly");
        assert_eq!(resp.routine.schedules, vec!["@hourly", "0 9 * * *"]);
    });
}
