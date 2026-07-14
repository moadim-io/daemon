//! Faceted filter over the loaded routines, and derived operational health.
//!
//! Pure, host-testable filtering of the loaded routines. The view binds a search
//! box, status facet, agent facet, and machine facet to a `RoutineFilter`; the
//! table and day timeline render `filter_routines(...)` instead of the raw list.
//! Best-practice (Airflow/Buildkite/GitHub Actions dashboards): free-text + facets
//! narrow a dense list, a live result count keeps the active filter legible, and
//! clicking a KPI tile cross-filters the detail table.

use chrono::{DateTime, Duration, Local};

use crate::schedule::{fires_within, next_fire_after};

pub use super::filter_distinct::{
    distinct_agents, distinct_machines_r, distinct_repositories, distinct_tags,
};
use super::model::Routine;

/// How far ahead a routine's next fire counts as "due soon" for the KPI tile.
pub(crate) const DUE_SOON_WINDOW_SECS: i64 = 3_600;

/// Enabled / disabled / dormant / due-soon status facet for routines.
/// `Dormant` means enabled but with an empty machines list — it will never fire.
/// `DueSoon` means enabled with a next fire within [`DUE_SOON_WINDOW_SECS`].
/// `HasFlags` means the routine has one or more open flags.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RoutineStatusFacet {
    #[default]
    All,
    Enabled,
    Disabled,
    Dormant,
    DueSoon,
    Snoozed,
    HasFlags,
    AgentUnregistered,
}

impl RoutineStatusFacet {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            RoutineStatusFacet::All => "all",
            RoutineStatusFacet::Enabled => "enabled",
            RoutineStatusFacet::Disabled => "disabled",
            RoutineStatusFacet::Dormant => "dormant",
            RoutineStatusFacet::DueSoon => "due",
            RoutineStatusFacet::Snoozed => "snoozed",
            RoutineStatusFacet::HasFlags => "flagged",
            RoutineStatusFacet::AgentUnregistered => "agent-unreg",
        }
    }

    #[must_use]
    pub fn from_str(s: &str) -> Self {
        match s {
            "enabled" => RoutineStatusFacet::Enabled,
            "disabled" => RoutineStatusFacet::Disabled,
            "dormant" => RoutineStatusFacet::Dormant,
            "due" => RoutineStatusFacet::DueSoon,
            "snoozed" => RoutineStatusFacet::Snoozed,
            "flagged" => RoutineStatusFacet::HasFlags,
            "agent-unreg" => RoutineStatusFacet::AgentUnregistered,
            _ => RoutineStatusFacet::All,
        }
    }
}

/// Sentinel select values for the machine facet. Real machine ids never collide
/// with these (no leading NUL in user-supplied names).
pub(crate) const RMACHINE_ANY: &str = "\u{0}any";
pub(crate) const RMACHINE_UNASSIGNED: &str = "\u{0}unassigned";

/// Machine facet for the routines filter.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum RoutineMachineFacet {
    #[default]
    Any,
    Unassigned,
    Machine(String),
}

impl RoutineMachineFacet {
    #[must_use]
    pub fn as_value(&self) -> String {
        match self {
            RoutineMachineFacet::Any => RMACHINE_ANY.to_string(),
            RoutineMachineFacet::Unassigned => RMACHINE_UNASSIGNED.to_string(),
            RoutineMachineFacet::Machine(m) => m.clone(),
        }
    }

    #[must_use]
    pub fn from_value(v: &str) -> Self {
        match v {
            RMACHINE_ANY => RoutineMachineFacet::Any,
            RMACHINE_UNASSIGNED => RoutineMachineFacet::Unassigned,
            other => RoutineMachineFacet::Machine(other.to_string()),
        }
    }
}

/// Agent facet: all agents, or one specific agent name.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum AgentFacet {
    #[default]
    All,
    Named(String),
}

impl AgentFacet {
    pub(crate) const AGENT_ALL: &'static str = "\u{0}all";

    #[must_use]
    pub fn as_value(&self) -> String {
        match self {
            AgentFacet::All => Self::AGENT_ALL.to_string(),
            AgentFacet::Named(a) => a.clone(),
        }
    }

    #[must_use]
    pub fn from_value(v: &str) -> Self {
        if v == Self::AGENT_ALL {
            AgentFacet::All
        } else {
            AgentFacet::Named(v.to_string())
        }
    }
}

/// Repository facet: all repositories, or one specific repository URL.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum RepositoryFacet {
    #[default]
    All,
    Named(String),
}

impl RepositoryFacet {
    pub(crate) const REPOSITORY_ALL: &'static str = "\u{0}all";

    #[must_use]
    pub fn as_value(&self) -> String {
        match self {
            RepositoryFacet::All => Self::REPOSITORY_ALL.to_string(),
            RepositoryFacet::Named(r) => r.clone(),
        }
    }

    #[must_use]
    pub fn from_value(v: &str) -> Self {
        if v == Self::REPOSITORY_ALL {
            RepositoryFacet::All
        } else {
            RepositoryFacet::Named(v.to_string())
        }
    }
}

/// Tag facet: all tags, or one specific tag.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum TagFacet {
    #[default]
    All,
    Named(String),
}

impl TagFacet {
    pub(crate) const TAG_ALL: &'static str = "\u{0}all";

    #[must_use]
    pub fn as_value(&self) -> String {
        match self {
            TagFacet::All => Self::TAG_ALL.to_string(),
            TagFacet::Named(t) => t.clone(),
        }
    }

    #[must_use]
    pub fn from_value(v: &str) -> Self {
        if v == Self::TAG_ALL {
            TagFacet::All
        } else {
            TagFacet::Named(v.to_string())
        }
    }
}

/// Combined free-text + faceted filter applied client-side to the loaded routines.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RoutineFilter {
    /// Free-text needle matched across title, agent, prompt, repositories,
    /// schedule, and `schedule_description`.
    pub query: String,
    pub status: RoutineStatusFacet,
    pub agent: AgentFacet,
    pub machine: RoutineMachineFacet,
    pub repository: RepositoryFacet,
    pub tag: TagFacet,
}

impl RoutineFilter {
    /// `true` when at least one facet is narrowing the list.
    #[must_use]
    pub fn is_active(&self) -> bool {
        !self.query.trim().is_empty()
            || self.status != RoutineStatusFacet::All
            || self.agent != AgentFacet::All
            || self.machine != RoutineMachineFacet::Any
            || self.repository != RepositoryFacet::All
            || self.tag != TagFacet::All
    }

    /// Does this routine survive the filter? Facets AND together.
    /// `now` and `window` are used only when the `DueSoon` status facet is active.
    #[must_use]
    pub fn matches(&self, r: &Routine, now: DateTime<Local>, window: Duration) -> bool {
        match self.status {
            RoutineStatusFacet::Enabled if !r.enabled => return false,
            RoutineStatusFacet::Disabled if r.enabled => return false,
            RoutineStatusFacet::Dormant if !(r.enabled && r.machines.is_empty()) => return false,
            RoutineStatusFacet::DueSoon
                if !(r.enabled && fires_within(&r.schedule, now, window)) =>
            {
                return false
            }
            RoutineStatusFacet::Snoozed if !is_routine_snoozed(r, now) => return false,
            RoutineStatusFacet::HasFlags if r.flag_count == 0 => return false,
            RoutineStatusFacet::AgentUnregistered if r.agent_registered => return false,
            _ => {}
        }
        match &self.agent {
            AgentFacet::Named(a) if r.agent != *a => return false,
            _ => {}
        }
        match &self.machine {
            RoutineMachineFacet::Unassigned if !r.machines.is_empty() => return false,
            RoutineMachineFacet::Machine(m) if !r.machines.iter().any(|x| x == m) => return false,
            _ => {}
        }
        match &self.repository {
            RepositoryFacet::Named(rp) if !r.repositories.iter().any(|x| x.repository == *rp) => {
                return false
            }
            _ => {}
        }
        match &self.tag {
            TagFacet::Named(t) if !r.tags.iter().any(|x| x == t) => return false,
            _ => {}
        }
        let q = self.query.trim().to_lowercase();
        if !q.is_empty() {
            let repos = r
                .repositories
                .iter()
                .map(|repo| repo.repository.to_lowercase())
                .collect::<Vec<_>>()
                .join(" ");
            let desc = r
                .schedule_description
                .as_deref()
                .unwrap_or_default()
                .to_lowercase();
            let tags = r
                .tags
                .iter()
                .map(|t| t.to_lowercase())
                .collect::<Vec<_>>()
                .join(" ");
            let hay = format!(
                "{} {} {} {} {} {}",
                r.title.to_lowercase(),
                r.agent.to_lowercase(),
                r.schedule.to_lowercase(),
                repos,
                desc,
                tags,
            );
            if !hay.contains(&q) {
                return false;
            }
        }
        true
    }
}

/// Returns the most-recent trigger timestamp across both manual and scheduled fires.
/// `None` means the routine has never been triggered.
///
/// Uses the max of the two Optional timestamps so that whichever kind fired most
/// recently is what the LAST FIRE column shows.
pub(crate) fn last_fire_at(r: &Routine) -> Option<u64> {
    match (r.last_manual_trigger_at, r.last_scheduled_trigger_at) {
        (None, None) => None,
        (Some(m), None) => Some(m),
        (None, Some(s)) => Some(s),
        (Some(m), Some(s)) => Some(m.max(s)),
    }
}

// ─── Health status ────────────────────────────────────────────────────────────

/// At-a-glance operational health derived from a routine's current fields.
/// Covers the same fault categories as the Overview attention-reason triage
/// plus the `Disabled` state so every row in the Routines table has a badge.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoutineHealth {
    /// Enabled but assigned to no machine — fires nowhere.
    Dormant,
    /// Enabled, has a machine, but the cron expression yields no future fire.
    DeadSchedule,
    /// Enabled, scheduled, has a machine, but agent config is missing.
    AgentMissing,
    /// `enabled: false` — intentionally paused.
    Disabled,
    /// Enabled, but system/policy paused for power saving — distinct from `Disabled` because it's
    /// not the user's choice and lifts on its own.
    PowerSaving,
    /// Enabled, scheduled, agent registered, but the agent snoozed its own scheduled fires.
    Snoozed,
    /// Enabled, scheduled, has a machine, agent registered — fully operational.
    Healthy,
}

impl RoutineHealth {
    /// Lower number = more urgent. Ascending sort puts broken rows first.
    pub(crate) fn priority(self) -> u8 {
        match self {
            RoutineHealth::Dormant => 0,
            RoutineHealth::DeadSchedule => 1,
            RoutineHealth::AgentMissing => 2,
            RoutineHealth::Disabled => 3,
            RoutineHealth::PowerSaving => 4,
            RoutineHealth::Snoozed => 5,
            RoutineHealth::Healthy => 6,
        }
    }

    /// Short uppercase label shown in the badge.
    pub(crate) fn badge(self) -> &'static str {
        match self {
            RoutineHealth::Dormant => "DORMANT",
            RoutineHealth::DeadSchedule => "DEAD SCHEDULE",
            RoutineHealth::AgentMissing => "AGENT MISSING",
            RoutineHealth::Disabled => "DISABLED",
            RoutineHealth::PowerSaving => "POWER SAVING",
            RoutineHealth::Snoozed => "SNOOZED",
            RoutineHealth::Healthy => "HEALTHY",
        }
    }

    /// CSS class string for the badge `<span>`.
    pub(crate) fn badge_class(self) -> &'static str {
        match self {
            RoutineHealth::Dormant => "health-badge dormant",
            RoutineHealth::DeadSchedule => "health-badge dead",
            RoutineHealth::AgentMissing => "health-badge agent-missing",
            RoutineHealth::Disabled => "health-badge disabled",
            RoutineHealth::PowerSaving => "health-badge power-saving",
            RoutineHealth::Snoozed => "health-badge snoozed",
            RoutineHealth::Healthy => "health-badge healthy",
        }
    }
}

/// Derive the operational health of a routine as of `now`.
///
/// Faults are checked in priority order — `Dormant` outranks `DeadSchedule`
/// which outranks `AgentMissing` — matching the Overview triage ordering.
/// `true` when the routine's scheduled fires are currently suppressed by a
/// snooze deadline or a non-zero skip-runs counter.
pub(crate) fn is_routine_snoozed(r: &Routine, now: DateTime<Local>) -> bool {
    r.snoozed_until
        .is_some_and(|until| (until as i64) > now.timestamp())
        || r.skip_runs.is_some_and(|runs| runs > 0)
}

/// Short human-readable detail for a snoozed routine's NEXT RUN cell:
/// "until HH:MM" for a timestamp snooze, or "N run(s) skipped" for a
/// counter snooze. Returns an empty string when neither applies.
pub(crate) fn snooze_detail(r: &Routine, now: DateTime<Local>) -> String {
    if let Some(until) = r.snoozed_until {
        let until_secs = until as i64;
        if until_secs > now.timestamp() {
            let remaining = (until_secs - now.timestamp()) as u64;
            return if remaining < 3_600 {
                format!("{}m left", remaining / 60)
            } else if remaining < 86_400 {
                format!("{}h left", remaining / 3_600)
            } else {
                format!("{}d left", remaining / 86_400)
            };
        }
    }
    if let Some(runs) = r.skip_runs {
        if runs > 0 {
            return format!("{runs} run{} skipped", if runs == 1 { "" } else { "s" });
        }
    }
    String::new()
}

#[must_use]
pub fn routine_health(r: &Routine, now: DateTime<Local>) -> RoutineHealth {
    if !r.enabled {
        return RoutineHealth::Disabled;
    }
    if r.power_saving {
        return RoutineHealth::PowerSaving;
    }
    if r.machines.iter().all(|m| m.trim().is_empty()) {
        return RoutineHealth::Dormant;
    }
    if next_fire_after(&r.schedule, now).is_none() {
        return RoutineHealth::DeadSchedule;
    }
    if !r.agent_registered {
        return RoutineHealth::AgentMissing;
    }
    if is_routine_snoozed(r, now) {
        return RoutineHealth::Snoozed;
    }
    RoutineHealth::Healthy
}

/// Tooltip for the row's "Run now" button, naming the reason a manual trigger would be refused.
#[must_use]
pub fn trigger_button_title(r: &Routine) -> &'static str {
    if !r.enabled {
        "Routine is disabled"
    } else if r.power_saving {
        "Routine is in power-saving mode"
    } else {
        "Run now"
    }
}

/// Routines surviving `filter`, preserving the input order.
#[must_use]
pub fn filter_routines(
    routines: &[Routine],
    filter: &RoutineFilter,
    now: DateTime<Local>,
    window: Duration,
) -> Vec<Routine> {
    routines
        .iter()
        .filter(|r| filter.matches(r, now, window))
        .cloned()
        .collect()
}

#[cfg(test)]
#[path = "filter_tests.rs"]
mod filter_tests;

#[cfg(test)]
#[path = "filter_distinct_tests.rs"]
mod filter_distinct_tests;

#[cfg(test)]
#[path = "filter_facet_codec_tests.rs"]
mod filter_facet_codec_tests;

#[cfg(test)]
#[path = "filter_health_tests.rs"]
mod filter_health_tests;
