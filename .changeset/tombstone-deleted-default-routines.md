---
"moadim": patch
---

fix(routines): tombstone a deleted built-in default so it stays deleted

Deleting a built-in default routine now records its slug in a tombstone
file (`removed_default_routines_path`), so the next startup's
`ensure_default_routines` no longer resurrects it enabled. Re-creating a
routine under a tombstoned default's title clears the tombstone, since
that is a deliberate "bring it back" signal.
