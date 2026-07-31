---
"moadim": patch
---

Enable `clippy::assigning_clones` and fix the one violation it surfaced: `svc_update` now uses `clone_from` instead of assigning a fresh `.clone()` onto the routine's existing `schedule` field.
