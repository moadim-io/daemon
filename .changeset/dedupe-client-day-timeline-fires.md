---
"moadim": patch
---

Dedupe the client's day-timeline fire-time math: `pages/routines/DayTimeline.tsx` had its own untested copy of the cron-to-fire-times logic (including the midnight-boundary seed trick) instead of the already-tested `fireTimesOnDay` used by the heatmap's day drill-down. Moved `fireTimesOnDay` into `lib/schedule.ts` as the single shared implementation (heatmap's `dayTimelineMath.ts` now re-exports it), pointed the routines page at it, and moved its tests to `schedule.test.ts`. No behavior change.
