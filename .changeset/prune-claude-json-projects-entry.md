---
"moadim": patch
---

fix(cleanup): prune stale `~/.claude.json` `projects` entry when reaping workbenches

The built-in `claude` agent's `setup` step seeds a per-workbench entry into
`~/.claude.json`, keyed by the workbench's absolute (always-unique) path, on
every run. Nothing ever pruned it once the workbench was reaped, so the file
grew by one dead entry per `claude` run, forever. Cleanup now removes the
matching `projects[<workbench>]` entry when it reaps a workbench directory,
using the same flock-guarded read -> modify -> atomic-replace pattern the
setup step already uses. (#430)
