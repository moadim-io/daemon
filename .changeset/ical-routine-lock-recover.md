---
"moadim": patch
---

### Fixed

- **`GET /routines/{id}.ics` no longer panics on a poisoned routine store
  lock.** `svc_ical_routine` locked the shared `RoutineStore` with
  `.lock().expect("routine store lock poisoned")`, unlike its sibling
  `svc_ical` (and every other store accessor) which already recovers via
  `LockRecover::lock_recover()`. Since the store is a process-wide singleton,
  any earlier panic while the lock was held anywhere in the daemon would
  permanently poison it, and this one remaining call site would then panic
  on every subsequent request to the per-routine iCal feed instead of
  degrading gracefully like the rest of the API surface. Switched it to
  `lock_recover()` to close that gap.
