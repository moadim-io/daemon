#![allow(
    clippy::wildcard_imports,
    clippy::too_many_arguments,
    clippy::needless_pass_by_value,
    dead_code,
    reason = "split-out module preserves existing code while keeping files under the linecheck limit"
)]
use super::*;

/// Return the routines matching `query`, filtered and sorted as requested.
///
/// The default query (no repository filter, sort by creation time ascending)
/// reproduces the previous behaviour, except each routine's `prompt` is omitted
/// unless `include_prompts` is `true`. The `repository` filter keeps routines
/// referencing a matching repository URL; `sort`/`order` control ordering.
pub fn svc_list(
    store: &RoutineStore,
    dir: &std::path::Path,
    query: &RoutineListQuery,
) -> Vec<RoutineResponse> {
    // Refresh from disk first so a routine pulled/edited on disk under a running daemon (including a
    // changed `machines` list) is reflected without a restart. Disk is the source of truth.
    crate::routine_storage::reload_store_from_dir(store, dir);
    let lock = store.lock_recover();
    let mut routines: Vec<Routine> = lock.values().cloned().collect();
    drop(lock);

    // Filter: keep routines with a repository URL containing the substring (case-insensitive).
    if let Some(needle) = query
        .repository
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        let needle = needle.to_lowercase();
        routines.retain(|routine| {
            routine
                .repositories
                .iter()
                .any(|repo| repo.repository.to_lowercase().contains(&needle))
        });
    }

    // Filter: keep only routines that target the current machine.
    if query.local_only.unwrap_or(false) {
        let me = crate::machine::current_machine();
        routines.retain(|routine| crate::machine::targets(&routine.machines, &me));
    }

    // Sort by the requested field. The routines come off a `HashMap`, whose
    // iteration order is unspecified, so equal sort keys would otherwise list
    // in an arbitrary, run-to-run order. Break ties on the stable routine id to
    // make the listing deterministic, and reverse the whole comparison (not the
    // sorted vector) for descending order so the id tiebreak stays consistent.
    let desc = query.order == SortOrder::Desc;
    routines.sort_by(|left, right| {
        let primary = match query.sort {
            RoutineSort::Created => left.created_at.cmp(&right.created_at),
            RoutineSort::Updated => left.updated_at.cmp(&right.updated_at),
            RoutineSort::Title => left.title.to_lowercase().cmp(&right.title.to_lowercase()),
            RoutineSort::Repository => repo_sort_key(left).cmp(&repo_sort_key(right)),
        };
        let ord = primary.then_with(|| left.id.cmp(&right.id));
        if desc {
            ord.reverse()
        } else {
            ord
        }
    });

    // Omit prompts by default: they are the largest field and rarely needed in a listing.
    // Blanking triggers `skip_serializing_if` on `Routine::prompt`, dropping it from the JSON.
    let include_prompts = query.include_prompts.unwrap_or(false);

    routines
        .into_iter()
        .map(|mut routine| {
            if !include_prompts {
                routine.prompt.clear();
            }
            RoutineResponse::from_routine(routine)
        })
        .collect()
}

/// Look up a routine by `id`, returning `NotFound` if it does not exist.
pub fn svc_get(
    store: &RoutineStore,
    dir: &std::path::Path,
    id: &str,
) -> Result<RoutineResponse, AppError> {
    // Refresh from disk first so a freshly-pulled or edited routine is visible without a restart.
    crate::routine_storage::reload_store_from_dir(store, dir);
    let routine = store
        .lock_recover()
        .get(id)
        .cloned()
        .ok_or(AppError::NotFound)?;
    Ok(RoutineResponse::from_routine(routine))
}
include!("svc_create.rs");
