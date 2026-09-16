#![allow(
    clippy::missing_docs_in_private_items,
    reason = "test helpers and fixtures do not need doc comments"
)]

use super::*;

#[test]
fn run_history_lists_each_live_manual_or_scheduled_workbench_once() {
    use crate::routines::run_history::{append_persisted_run, PersistedRun};

    let _home = TempHome::set();
    let title = "Scheduled Manual History ZZQ";
    let slug = slugify(title);
    let store = new_store();
    store
        .lock()
        .unwrap()
        .insert("id".into(), make_routine("id", title));

    let manual = format!("{slug}-2000_101");
    let scheduled = format!("{slug}-2000_202");
    let workbenches = crate::paths::workbenches_dir();
    std::fs::create_dir_all(workbenches.join(&manual)).unwrap();
    std::fs::create_dir_all(workbenches.join(&scheduled)).unwrap();

    // Cleanup writes durable history before it removes the workbench. If removal fails, the next
    // listing sees both representations of this manual run and must not report it twice.
    append_persisted_run(
        "id",
        &PersistedRun {
            workbench: manual.clone(),
            started_at: 2000,
            finished_at: 2001,
            status: RunStatus::Success,
            exit_code: Some(0),
        },
    );

    let per_routine = svc_list_runs(&store, "id").unwrap();
    let fleet = svc_list_all_runs(&store, None);
    for names in [
        per_routine
            .iter()
            .map(|run| run.workbench.as_str())
            .collect::<std::collections::BTreeSet<_>>(),
        fleet
            .iter()
            .map(|run| run.workbench.as_str())
            .collect::<std::collections::BTreeSet<_>>(),
    ] {
        assert_eq!(
            names,
            std::collections::BTreeSet::from([manual.as_str(), scheduled.as_str()])
        );
    }
    assert_eq!(per_routine.len(), 2);
    assert_eq!(fleet.len(), 2);
}
