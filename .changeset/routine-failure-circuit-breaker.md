---
"moadim": minor
---

### Added

Opt-in failure circuit-breaker for routines (#521): set a routine's new `failure_threshold` to
auto-disable it (and stop scheduling further runs) after that many consecutive failed-or-unknown
runs, instead of retrying forever. `consecutive_failures` and `auto_disabled_reason` are tracked
per routine and surfaced on `GET /routines`; manually re-enabling a routine resets the counter and
clears the reason. Leaving `failure_threshold` unset (or `0`) keeps today's unlimited-retry
behavior. UI surfacing is left as a follow-up.
