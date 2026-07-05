---
"moadim": patch
---

fix: don't duplicate a workbench's `runs.log` entry when its removal is retried

The reap sweep persists a finished workbench's outcome to the routine's durable
`runs.log` *before* removing its directory, so `svc_list_runs` still knows
about the run once the workbench is gone. If that `remove_dir_all` then fails
(a permission hiccup, a file still open, a crash), the workbench survives and
gets expired again on the next sweep — which re-persisted the same run,
appending a duplicate `runs.log` entry every sweep the removal kept failing.
The `persist` step now skips workbenches that already have a matching record.
