#![allow(
    clippy::missing_docs_in_private_items,
    reason = "test fixtures intentionally trade docs for compact call sites"
)]

use crate::routines::{Repository, Routine};

pub(crate) fn routine_fixture(id: &str, title: &str) -> RoutineFixture {
    RoutineFixture {
        id: id.to_string(),
        title: title.to_string(),
        prompt: "task".to_string(),
        repositories: vec![Repository {
            repository: "https://example.com/r.git".to_string(),
            branch: Some("main".to_string()),
            auto_pull: true,
        }],
        enabled: true,
        disabled_reason: None,
        created_at: 5,
        updated_at: 6,
    }
}

pub(crate) struct RoutineFixture {
    id: String,
    title: String,
    prompt: String,
    repositories: Vec<Repository>,
    enabled: bool,
    disabled_reason: Option<String>,
    created_at: u64,
    updated_at: u64,
}

impl RoutineFixture {
    pub(crate) fn prompt(mut self, prompt: &str) -> Self {
        self.prompt = prompt.to_string();
        self
    }

    pub(crate) fn no_repositories(mut self) -> Self {
        self.repositories = vec![];
        self
    }

    pub(crate) fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    pub(crate) fn disabled_reason(mut self, reason: &str) -> Self {
        self.disabled_reason = Some(reason.to_string());
        self
    }

    pub(crate) fn times(mut self, created_at: u64, updated_at: u64) -> Self {
        self.created_at = created_at;
        self.updated_at = updated_at;
        self
    }

    pub(crate) fn build(self) -> Routine {
        Routine {
            model: None,
            id: self.id,
            schedule: "@daily".to_string(),
            schedules: vec![],
            title: self.title,
            agent: "claude".to_string(),
            prompt: self.prompt,
            goal: None,
            repositories: self.repositories,
            machines: vec![crate::machine::current_machine()],
            enabled: self.enabled,
            disabled_reason: self.disabled_reason,
            source: "managed".to_string(),
            created_at: self.created_at,
            updated_at: self.updated_at,
            last_manual_trigger_at: None,
            last_scheduled_trigger_at: None,
            snoozed_until: None,
            skip_runs: None,
            power_saving: false,
            power_saving_exempt: false,
            tags: vec![],
            ttl_secs: None,
            max_runtime_secs: None,
            env: std::collections::HashMap::new(),
            auto_disabled_reason: None,
            consecutive_failures: 0,
            failure_threshold: None,
            notifications: Default::default(),
        }
    }
}
