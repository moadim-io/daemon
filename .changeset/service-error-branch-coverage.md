---
"moadim": patch
---

Add tests for the 8 previously-uncovered `AppError::Internal` error branches in `src/routines/service.rs` (`svc_update`'s goal validation, `svc_trigger_scheduled`'s snooze/skip-runs write paths, `svc_snooze`, `svc_create_flag`, and `svc_resolve_flag`), closing `service.rs` to 100% region coverage. Test-only, no behavior change.
