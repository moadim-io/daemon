//! Distinct facet-option helpers (agents / machines / repositories / tags) used
//! to populate the filter bar's dropdown options from the loaded routines.

use std::collections::BTreeSet;

use super::model::Routine;

/// Distinct agent names across all routines, sorted.
#[must_use]
pub fn distinct_agents(routines: &[Routine]) -> Vec<String> {
    let mut set: BTreeSet<String> = BTreeSet::new();
    for r in routines {
        set.insert(r.agent.clone());
    }
    set.into_iter().collect()
}

/// Distinct machine ids across all routines, sorted.
#[must_use]
pub fn distinct_machines_r(routines: &[Routine]) -> Vec<String> {
    let mut set: BTreeSet<String> = BTreeSet::new();
    for r in routines {
        for m in &r.machines {
            set.insert(m.clone());
        }
    }
    set.into_iter().collect()
}

/// Distinct repository URLs across all routines, sorted.
#[must_use]
pub fn distinct_repositories(routines: &[Routine]) -> Vec<String> {
    let mut set: BTreeSet<String> = BTreeSet::new();
    for r in routines {
        for repo in &r.repositories {
            set.insert(repo.repository.clone());
        }
    }
    set.into_iter().collect()
}

/// Distinct tags across all routines, sorted.
#[must_use]
pub fn distinct_tags(routines: &[Routine]) -> Vec<String> {
    let mut set: BTreeSet<String> = BTreeSet::new();
    for r in routines {
        for tag in &r.tags {
            set.insert(tag.clone());
        }
    }
    set.into_iter().collect()
}
