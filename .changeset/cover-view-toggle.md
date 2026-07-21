---
"moadim": patch
---

test(client): add coverage for the routines-page `ViewToggle` (list/calendar/day switcher)

`ViewToggle.tsx` had zero test coverage (`pnpm --filter client test:coverage`
showed 25% statements / 0% branches, lines 16-23 uncovered) even though it
carries real interactive logic — which button renders active and which view
value gets passed back on click. Adds `ViewToggle.test.tsx` covering: all
three buttons render with their labels, only the current view's button gets
the `active` class, and clicking a button calls `onSetView` with that
button's view (including re-clicking the already-active one, which is a
no-op passthrough rather than a toggle-off like `StatsBar`'s facets).

No behavior change — test-only. `ViewToggle.tsx` is now at 100% coverage.
