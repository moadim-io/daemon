---
"moadim": minor
---

### Added

UI: surface the failure circuit-breaker (#521) on the Routines and Overview pages. A routine
auto-disabled by repeated failures now shows a distinct AUTO-DISABLED health badge (tooltip
carries the daemon's actual `auto_disabled_reason`) instead of being indistinguishable from a
routine someone paused on purpose, gets its own filter facet and Routines-page stat tile, and
now appears in the Overview's NEEDS ATTENTION panel — previously auto-disabled routines were
silently excluded there along with intentionally-paused ones. An enabled routine nearing its
`failure_threshold` also gets an inline "N/M failures" chip (amber, red on the failure that would
trip the breaker) as an early warning before it auto-disables.
