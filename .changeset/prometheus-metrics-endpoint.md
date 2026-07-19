---
"moadim": minor
---

feat(observability): add `GET /api/v1/metrics`, a Prometheus text-exposition endpoint (#414)

Exposes `moadim_uptime_seconds`, `moadim_build_info`, `moadim_active_sessions`,
`moadim_workbench_bytes`, `moadim_runs_total{status=...}`,
`moadim_run_duration_seconds` (histogram), `moadim_cleanup_removed_total`, and
`moadim_cleanup_freed_bytes_total`. Run counts/durations and active sessions are
derived at scrape time from the same durable run history (`runs.log` + live
workbenches) and live tmux session count the REST "recent runs" view and the
concurrency cap already read, rather than a second, parallel counter that could
drift from it. Cleanup-sweep totals are tracked as process-lifetime atomics,
incremented at the one function both the periodic sweep and the on-demand
`POST /routines/cleanup` route already funnel through, so they reflect real
sweeps and not just on-demand snapshots. `GET /health` is unchanged — it stays
the cheap liveness probe, `/metrics` is the richer scrape surface.
