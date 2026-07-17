---
"moadim": patch
---

test(ui,client): cover `healthBadge`/`healthBadgeClass` (and their Rust `RoutineHealth` counterparts) for every variant

`RoutineHealth::badge()`/`badge_class()` in `ui/src/routines/filter.rs` and their 1:1 TypeScript
port `healthBadge`/`healthBadgeClass` in `client/src/pages/routines/filter.ts` were the only
exported health-rendering functions with no test on either side — `priority()`/`healthPriority`
already had one, but the badge label and CSS class returned for each of the 7 `RoutineHealth`
variants were unverified. A typo or copy-paste duplicate (e.g. two variants sharing a CSS class,
or a mismatched label) would have shipped silently to the ROUTINES table's health badge. Both
sides now assert the exact rendered string per variant and that labels/classes stay unique across
variants, mirroring the existing `health_priority_order_dormant_most_urgent`/`healthPriority`
tests. No behavior change.
