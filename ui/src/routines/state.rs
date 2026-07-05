//! Page/view/modal state, group-by, sorting, and the `RState` reducer.

use std::collections::BTreeSet;
use std::rc::Rc;

use chrono::{DateTime, Local};
use yew::Reducible;

use crate::schedule::next_fire_after;

use super::filter::{
    last_fire_at, routine_health, AgentFacet, RepositoryFacet, RoutineFilter, RoutineMachineFacet,
    RoutineStatusFacet, TagFacet,
};
use serde::{Deserialize, Serialize};

use super::model::{LockStatus, Routine};

/// Route query used to deep-link straight to a routine's HISTORY page (e.g. from the overview
/// page's RECENT RUNS panel), instead of landing on the plain routine list.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RoutineHistoryQuery {
    pub history: String,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub enum RPage {
    #[default]
    List,
    New,
    Logs(String),
    History(String),
    Flags(String),
    /// Pre-filled create form cloned from an existing routine.
    Clone(Box<Routine>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum RModal {
    None,
    Edit(String),
    ConfirmDelete { id: String, title: String },
    ConfirmBulkDelete { count: usize },
}

/// How the list page presents routines: a table, or a month calendar of upcoming fire times.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum RView {
    #[default]
    Table,
    Calendar,
    Day,
}

/// Column the routine table is sorted by (click-to-sort).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RCol {
    Title,
    NextRun,
    LastFire,
    Agent,
    Health,
    Enabled,
    Updated,
}

/// Sort direction for the routine table.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RDir {
    #[default]
    Asc,
    Desc,
}

impl RDir {
    /// Toggle to the opposite direction.
    #[must_use]
    pub fn flip(self) -> Self {
        match self {
            RDir::Asc => RDir::Desc,
            RDir::Desc => RDir::Asc,
        }
    }
}

// ─── Group-by ────────────────────────────────────────────────────────────────

/// Dimension used to partition the Routines table into labelled sections.
/// Orthogonal to faceted filtering and column sorting — composes with both.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RGroupBy {
    #[default]
    None,
    /// Group by the routine's agent (claude, codex, …).
    Agent,
    /// Group by target machine; routines with no machine share an `(unassigned)` section.
    Machine,
    /// Group by enabled/disabled status.
    Status,
    /// Group by the derived health badge (Healthy, Snoozed, Dormant, …).
    Health,
}

impl RGroupBy {
    /// Stable token stored as the `<select>` option value.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            RGroupBy::None => "none",
            RGroupBy::Agent => "agent",
            RGroupBy::Machine => "machine",
            RGroupBy::Status => "status",
            RGroupBy::Health => "health",
        }
    }

    /// Parse a token back to a variant, defaulting to `None` for unknown values.
    #[must_use]
    pub fn from_str(s: &str) -> Self {
        match s {
            "agent" => RGroupBy::Agent,
            "machine" => RGroupBy::Machine,
            "status" => RGroupBy::Status,
            "health" => RGroupBy::Health,
            _ => RGroupBy::None,
        }
    }

    /// Short human label shown in the selector dropdown.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            RGroupBy::None => "None",
            RGroupBy::Agent => "Agent",
            RGroupBy::Machine => "Machine",
            RGroupBy::Status => "Status",
            RGroupBy::Health => "Health",
        }
    }
}

/// Group key for a single routine under the given dimension.
#[must_use]
pub fn routine_group_key(r: &Routine, by: RGroupBy) -> String {
    match by {
        RGroupBy::None => String::new(),
        RGroupBy::Agent => r.agent.clone(),
        RGroupBy::Machine => r
            .machines
            .first()
            .cloned()
            .unwrap_or_else(|| "(unassigned)".to_string()),
        RGroupBy::Status => {
            if r.enabled {
                "Enabled".to_string()
            } else {
                "Disabled".to_string()
            }
        }
        RGroupBy::Health => routine_health(r, Local::now()).badge().to_string(),
    }
}

/// Partition `routines` into `(group_label, routines_in_group)` pairs sorted
/// alphabetically by label. Within each group the input order is preserved.
/// When `by` is `None`, returns a single pair with an empty label.
#[must_use]
pub fn group_routines(routines: &[Routine], by: RGroupBy) -> Vec<(String, Vec<Routine>)> {
    use std::collections::BTreeMap;
    if by == RGroupBy::None {
        return vec![(String::new(), routines.to_vec())];
    }
    let mut map: BTreeMap<String, Vec<Routine>> = BTreeMap::new();
    for r in routines {
        map.entry(routine_group_key(r, by))
            .or_default()
            .push(r.clone());
    }
    map.into_iter().collect()
}

/// Return `routines` sorted by `col` in `dir` order. When `col` is `None` the
/// server/insertion order is preserved. Ties break by id for a stable sort.
#[must_use]
pub fn sort_routines(
    mut routines: Vec<Routine>,
    col: Option<RCol>,
    dir: RDir,
    now: DateTime<Local>,
) -> Vec<Routine> {
    let col = match col {
        Some(c) => c,
        None => return routines,
    };
    routines.sort_by(|a, b| {
        let primary = match col {
            RCol::Title => a.title.to_lowercase().cmp(&b.title.to_lowercase()),
            RCol::Agent => a.agent.to_lowercase().cmp(&b.agent.to_lowercase()),
            RCol::Enabled => a.enabled.cmp(&b.enabled),
            RCol::Updated => a.updated_at.cmp(&b.updated_at),
            RCol::Health => routine_health(a, now)
                .priority()
                .cmp(&routine_health(b, now).priority()),
            RCol::LastFire => last_fire_at(a).cmp(&last_fire_at(b)),
            RCol::NextRun => {
                let next_of = |r: &Routine| {
                    if r.enabled {
                        next_fire_after(&r.schedule, now)
                    } else {
                        None
                    }
                };
                match (next_of(a), next_of(b)) {
                    (Some(ta), Some(tb)) => ta.cmp(&tb),
                    (Some(_), None) => std::cmp::Ordering::Less,
                    (None, Some(_)) => std::cmp::Ordering::Greater,
                    (None, None) => std::cmp::Ordering::Equal,
                }
            }
        };
        let directed = if dir == RDir::Desc {
            primary.reverse()
        } else {
            primary
        };
        directed.then_with(|| a.id.cmp(&b.id))
    });
    routines
}

#[derive(Debug, Clone, PartialEq)]
pub struct RState {
    pub routines: Vec<Routine>,
    pub loading: bool,
    pub page: RPage,
    pub modal: RModal,
    pub view: RView,
    /// Active faceted filter.
    pub filter: RoutineFilter,
    /// Column the table is sorted by (`None` = natural order).
    pub sort_col: Option<RCol>,
    /// Direction of the active column sort.
    pub sort_dir: RDir,
    /// IDs of currently selected routines (multiselect for bulk actions).
    pub selected: BTreeSet<String>,
    /// Active group-by dimension; `None` renders a flat list.
    pub group_by: RGroupBy,
    /// Most recently fetched global lock status; `None` until the first fetch completes.
    pub lock_status: Option<LockStatus>,
    /// This machine's resolved name from the daemon, used to default the machine facet.
    pub current_machine: Option<String>,
}

impl Default for RState {
    fn default() -> Self {
        Self {
            routines: vec![],
            loading: true,
            page: RPage::List,
            modal: RModal::None,
            view: RView::default(),
            filter: RoutineFilter::default(),
            sort_col: None,
            sort_dir: RDir::default(),
            selected: BTreeSet::new(),
            group_by: RGroupBy::default(),
            lock_status: None,
            current_machine: None,
        }
    }
}

pub enum RAction {
    Loaded(Vec<Routine>),
    GoToNew,
    GoToList,
    GoToLogs(String),
    GoToHistory(String),
    GoToFlags(String),
    /// Open the create form pre-filled with a copy of the named routine.
    GoToClone(String),
    OpenEdit(String),
    OpenConfirmDelete {
        id: String,
        title: String,
    },
    OpenConfirmBulkDelete,
    CloseModal,
    SetView(RView),
    SetQuery(String),
    SetStatusFacet(RoutineStatusFacet),
    SetAgentFacet(AgentFacet),
    SetMachineFacet(RoutineMachineFacet),
    SetRepositoryFacet(RepositoryFacet),
    SetTagFacet(TagFacet),
    ClearFilters,
    /// Change the group-by dimension for the table view.
    SetGroupBy(RGroupBy),
    SortByCol(RCol),
    Upsert(Box<Routine>),
    Remove(String),
    /// Remove multiple routines after a confirmed bulk delete.
    RemoveMany(Vec<String>),
    /// Toggle one routine in/out of the selection set.
    SelectRoutine(String),
    /// Select exactly the given (visible/filtered) routine ids.
    SelectAll(Vec<String>),
    /// Clear the entire selection.
    ClearSelection,
    /// Received updated lock status from the server.
    LockStatusLoaded(LockStatus),
    /// Resolved current machine name received from the daemon; defaults machine facet to it.
    CurrentMachineLoaded(String),
}

impl Reducible for RState {
    type Action = RAction;

    fn reduce(self: Rc<Self>, action: Self::Action) -> Rc<Self> {
        let mut s = (*self).clone();
        match action {
            RAction::Loaded(r) => {
                // Drop selections for routines that no longer exist after a reload.
                let ids: BTreeSet<&String> = r.iter().map(|x| &x.id).collect();
                s.selected.retain(|id| ids.contains(id));
                s.routines = r;
                s.loading = false;
            }
            RAction::GoToNew => s.page = RPage::New,
            RAction::GoToList => s.page = RPage::List,
            RAction::GoToLogs(id) => s.page = RPage::Logs(id),
            RAction::GoToHistory(id) => s.page = RPage::History(id),
            RAction::GoToFlags(id) => s.page = RPage::Flags(id),
            RAction::GoToClone(id) => {
                if let Some(source) = s.routines.iter().find(|x| x.id == id) {
                    s.page = RPage::Clone(Box::new(source.clone()));
                }
            }
            RAction::OpenEdit(id) => s.modal = RModal::Edit(id),
            RAction::OpenConfirmDelete { id, title } => {
                s.modal = RModal::ConfirmDelete { id, title }
            }
            RAction::OpenConfirmBulkDelete => {
                s.modal = RModal::ConfirmBulkDelete {
                    count: s.selected.len(),
                };
            }
            RAction::CloseModal => s.modal = RModal::None,
            RAction::SetView(view) => s.view = view,
            RAction::SetQuery(q) => s.filter.query = q,
            RAction::SetStatusFacet(st) => s.filter.status = st,
            RAction::SetAgentFacet(ag) => s.filter.agent = ag,
            RAction::SetMachineFacet(m) => s.filter.machine = m,
            RAction::SetRepositoryFacet(rp) => s.filter.repository = rp,
            RAction::SetTagFacet(t) => s.filter.tag = t,
            RAction::ClearFilters => s.filter = RoutineFilter::default(),
            RAction::SetGroupBy(by) => s.group_by = by,
            RAction::SortByCol(col) => {
                if s.sort_col == Some(col) {
                    s.sort_dir = s.sort_dir.flip();
                } else {
                    s.sort_col = Some(col);
                    s.sort_dir = RDir::Asc;
                }
            }
            RAction::Upsert(routine) => {
                let routine = *routine;
                if let Some(i) = s.routines.iter().position(|x| x.id == routine.id) {
                    s.routines[i] = routine;
                } else {
                    s.routines.push(routine);
                }
            }
            RAction::Remove(id) => {
                s.routines.retain(|x| x.id != id);
                s.selected.remove(&id);
            }
            RAction::RemoveMany(ids) => {
                let drop: BTreeSet<&String> = ids.iter().collect();
                s.routines.retain(|r| !drop.contains(&r.id));
                s.selected.retain(|id| !drop.contains(id));
            }
            RAction::SelectRoutine(id) => {
                if !s.selected.remove(&id) {
                    s.selected.insert(id);
                }
            }
            RAction::SelectAll(ids) => {
                s.selected = ids.into_iter().collect();
            }
            RAction::ClearSelection => {
                s.selected.clear();
            }
            RAction::LockStatusLoaded(status) => {
                s.lock_status = Some(status);
            }
            RAction::CurrentMachineLoaded(name) => {
                s.current_machine = Some(name.clone());
                s.filter.machine = RoutineMachineFacet::Machine(name);
            }
        }
        s.into()
    }
}

#[cfg(test)]
#[path = "state_tests.rs"]
mod state_tests;

#[cfg(test)]
#[path = "state_group_by_tests.rs"]
mod state_group_by_tests;
