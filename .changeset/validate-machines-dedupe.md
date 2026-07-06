---
"moadim": patch
---

Remove a dead duplicate `validate_machines` helper left over after merging with `main`, which already validates machines via `routines::service_validate::validate_machines` (#600).
