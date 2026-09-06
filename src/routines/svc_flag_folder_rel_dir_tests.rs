#![allow(
    clippy::wildcard_imports,
    clippy::too_many_arguments,
    clippy::needless_pass_by_value,
    dead_code,
    reason = "split-out module preserves existing code while keeping files under the linecheck limit"
)]
use super::*;

/// Regression for #1514: flags must be keyed by the routine's actual on-disk directory
/// (`routine_rel_dir`), not `slugify(&routine.title)`. Before the fix, a routine moved into a
/// folder had its flags silently written to (and read from) a phantom top-level
/// `routines/{slug}/flags/` dir that `compose_prompt` never looked at, so raised flags never
/// reached the agent and never left `svc_list_flags`/`flag_count` even after being resolved.
#[test]
fn svc_create_flag_visible_after_move_to_folder() {
    let _home = TempHome::set();
    let store = new_store();
    let created = svc_create(&store, create_req_with_title("Svc Flag Folder ZZZ")).unwrap();
    let id = created.routine.id;

    svc_move(
        &store,
        &id,
        MoveRoutineRequest {
            folder: Some("ops".into()),
            slug: "svc-flag-folder-zzz".into(),
        },
    )
    .unwrap();

    let flag = svc_create_flag(&store, &id, "bug", "stuck in the wrong dir", "general").unwrap();

    // Landed under the routine's real (foldered) directory, not a phantom title-slug dir.
    assert!(crate::paths::routine_flags_dir("ops/svc-flag-folder-zzz")
        .join(&flag.filename)
        .exists());

    let flags = svc_list_flags(&store, &id).unwrap();
    assert_eq!(flags.len(), 1);
    assert_eq!(flags[0].filename, flag.filename);

    let routine = store.lock_recover().get(&id).cloned().unwrap();
    assert_eq!(RoutineResponse::from_routine(routine).flag_count, 1);

    svc_resolve_flag(&store, &id, &flag.filename).unwrap();
    assert!(svc_list_flags(&store, &id).unwrap().is_empty());
}
