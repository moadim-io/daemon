---
"moadim": patch
---

Split `src/routines/service_trigger_tests.rs` (742 lines) into three files to clear the pre-push `linecheck --max-lines 500` gate: trigger/scheduled-fire tests stay in place, snooze/lock/crontab-sync tests move to a new `service_trigger_snooze_tests.rs`, and the ANSI-stripping/log-tail tests move into the existing `service_logs_tests.rs`. No behavior change; same 231 test cases still run.
