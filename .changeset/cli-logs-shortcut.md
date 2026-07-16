---
"moadim": patch
---

feat(cli): add `moadim logs <id>` as a top-level shortcut for `moadim routines logs <id>`

The daemon has served a routine's newest run log over `GET /api/v1/routines/{id}/logs` since
`svc_logs()` landed, reachable from the CLI only via `moadim routines logs <id>`. `trigger`
already gets a bare top-level shortcut alongside its `routines trigger` form; `logs` did not
(issue #332). `moadim logs <id>` now mirrors that duality: same route, same exit-code
conventions (`0` on success including an empty not-yet-run log, non-zero on an unknown routine,
`3` when no daemon is reachable), documented in `--help` and shell completions.
