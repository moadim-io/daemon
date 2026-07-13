---
"moadim": patch
---

Add host-side unit tests for the day timeline's `fire_times` (`ui/src/day_timeline.rs`), covering multi-fire schedules, the midnight-boundary seed, adjacent-day filtering, unparseable schedules, and the `MAX_FIRES` cap. This logic previously had no test module, unlike every other pure-logic file in the `ui` crate. No behavior change.
