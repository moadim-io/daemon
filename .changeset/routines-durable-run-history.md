---
"moadim": minor
---

feat(routines): persist run history past workbench TTL reaping

`svc_list_runs`/`svc_list_all_runs` (and the HISTORY page / RECENT RUNS
panel that read them) used to show only runs whose workbench directory
was still on disk — once TTL-reaped, a run's outcome vanished. The reaper
now appends a compact record (workbench, timestamps, status, exit code)
to each routine's `runs.log` right before removing its workbench, so a
routine's run history survives past its configured retention window (the
`agent.log` body itself is still discarded, since retaining full logs
forever isn't the retention knob's job).
