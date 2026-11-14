---
"moadim": minor
---

Failure notification hooks now say where a run failed and link to it: webhooks get `machine`, a numeric `exit_code` (`null` for a timeout or unknown outcome) and `run_url`; command hooks get `MOADIM_MACHINE`, `MOADIM_EXIT_CODE` (unset when there is no numeric exit) and `MOADIM_RUN_URL`. Set `public_base_url` in the global `[notifications]` table to override the URL base.
