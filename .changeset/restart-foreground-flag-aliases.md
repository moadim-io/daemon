---
"moadim": patch
---

### Fixed

`moadim restart -f`/`--foreground` now relaunches attached to the terminal
instead of silently falling back to a detached restart — `-f`/`--foreground`
are now accepted as aliases of `-i`/`--interactive` on `restart`, matching
`start`'s existing behavior.
