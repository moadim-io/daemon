---
"moadim": patch
---

Served run logs now strip ANSI escapes with `anstream`'s VT parser instead of a hand-rolled one, so charset designations (`ESC ( B` from `tput sgr0`) and DCS payloads no longer leak into `svc_logs` / `svc_run_log` output.
