use crate::routines::service::service_trigger::power_saving_block_reason;

#[test]
fn system_power_saving_blocks_non_exempt_routine() {
    let mut routine = make_routine("system-power-saving-id", "System Power Saving Test", 1, 1);
    routine.power_saving_exempt = false;

    let reason = power_saving_block_reason(&routine, true);

    assert_eq!(reason, Some("system power saving is active"));
}

#[test]
fn system_power_saving_does_not_block_exempt_routine() {
    let mut routine = make_routine(
        "system-power-saving-exempt-id",
        "System Power Saving Exempt",
        1,
        1,
    );
    routine.power_saving_exempt = true;

    let reason = power_saving_block_reason(&routine, true);

    assert_eq!(reason, None);
}

#[test]
fn explicit_per_routine_power_saving_still_blocks_exempt_routine() {
    let mut routine = make_routine(
        "explicit-power-saving-id",
        "Explicit Power Saving Exempt",
        1,
        1,
    );
    routine.power_saving_exempt = true;
    routine.power_saving = true;

    let reason = power_saving_block_reason(&routine, true);

    assert_eq!(reason, Some("routine is in power-saving mode"));
}
