---
"moadim": patch
---

test(cli): cover `restart --quiet` skipping the endpoint-hint block

`restart(json, quiet)` never had a test exercising `quiet=true` — every existing case (parse
tests aside) only called `restart(_, false)`, so the `if !quiet { report_endpoints(); }` branch
that suppresses the UI/stop/logs hints was unverified behavior. Adds
`restart_quiet_skips_endpoint_hints_when_none_running`, mirroring the existing
`restart_json_skips_human_text_when_none_running` case. Test-only, no behavior change.
