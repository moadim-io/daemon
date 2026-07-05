---
"moadim": patch
---

fix(routines): dedupe `tags` on create/update, mirroring `machines`

`validate_tags` trimmed and rejected blank entries but never collapsed
duplicates, unlike its `validate_machines` sibling. A duplicate (or
whitespace-padded repeat) tag such as `["nightly", "nightly"]` or
`["nightly", " nightly "]` persisted verbatim, rendering as a doubled chip
in the routine row and an inflated tag list for a label that names one
concept once. `validate_tags` now dedupes on the trimmed value, keeping the
first occurrence, matching `validate_machines`'s existing behavior.
