---
"moadim": patch
---

Split `src/routines/service_trigger.rs` (→ `service_run_files.rs`) and `src/cli.rs` (→ `cli_restart.rs`) to satisfy the 500-line pre-push gate, which two independently-passing PRs had combined to exceed (`linecheck` isn't a required status check on the branch ruleset, so neither merge was blocked by it). No behavior change.
