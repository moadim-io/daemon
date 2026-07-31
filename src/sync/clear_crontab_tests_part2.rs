
#[test]
fn clear_on_an_absent_crontab_succeeds_with_zero() {
    let shim = CronShim::new(None);
    assert_eq!(clear_managed_crontab_blocks().unwrap(), 0);
    assert_eq!(shim.store_contents(), "", "nothing was written");
}

#[test]
fn clear_managed_crontab_blocks_errors_on_read_failure() {
    // A failing shim makes `read_crontab` return Err, exercising the `?` early on.
    let _shim = CronShim::failing();
    let err = clear_managed_crontab_blocks().unwrap_err();
    match err {
        SyncError::CrontabCommand(msg) => assert!(msg.contains("boom"), "unexpected msg: {msg}"),
        SyncError::Io(io) => panic!("expected CrontabCommand, got Io({io})"),
    }
}

#[test]
fn clear_managed_crontab_blocks_errors_on_write_failure() {
    // The initial crontab contains the managed block, so `updated != current` after removal and
    // `write_crontab` is called. The write-failing shim makes that call return Err.
    let initial =
        "# BEGIN MOADIM-ROUTINES\n* * * * * /x # moadim-routine:r1\n# END MOADIM-ROUTINES\n";
    let _shim = CronShim::write_fails(initial);
    let err = clear_managed_crontab_blocks().unwrap_err();
    assert!(
        matches!(err, SyncError::CrontabCommand(_)),
        "expected CrontabCommand error from write failure, got: {err:?}"
    );
}
