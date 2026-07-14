---
"moadim": minor
---

feat(routines): expose the global concurrency cap through the UI/REST

`MOADIM_MAX_CONCURRENT_RUNS` was previously only configurable via the environment variable. A
new `GET`/`PUT /config/max-concurrent-runs` REST endpoint and a settings-page card now let the
cap be viewed and changed at runtime, persisted to `~/.config/moadim/machine.local.toml`
(gitignored, machine-local, same tier as the existing machine-name override). Precedence:
`MOADIM_MAX_CONCURRENT_RUNS` env var (ops/CI) > the persisted UI/REST override > unbounded.
Takes effect on the next trigger check — no restart required.
