---
"moadim": patch
---

fix(cleanup): `cron_interval_secs` now applies the same multi-fire minimum-gap sampling to the `cron-union` path, not just the `croner` fallback. Routing schedule math through `cron-union` (#1322) reintroduced the exact "next two fires from now" bug just fixed for the `croner` path (#1323): for an unevenly-spaced multi-fire-per-day schedule like `"0,30 9 * * *"`, the TTL/max-runtime ceiling still flipped between 1800s and 3600s depending on wall-clock time, since `cron-union` compiles almost every schedule (only `@keyword` and 7-field expressions fall back to `croner`). Both branches now go through a shared `min_gap_secs` helper.
