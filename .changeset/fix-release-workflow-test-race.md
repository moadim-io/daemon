---
"moadim": patch
---

fix(ci): stop `publish.yml`/`release.yml` from racing their own redundant `test.yml` re-run on the automated release path (#1099)

Both workflows gated a redundant `lint`/`test` re-run on `github.event_name == 'push'`, meant to skip it when called from `auto-release.yml` on a verified version bump. A nested `workflow_call` inherits `github.event_name` from the chain's originating event, so it read `push` there too and the guard never actually skipped anything — every version bump ran three concurrent `test.yml` calls sharing one `test-<ref>` concurrency group with `cancel-in-progress: true`, and `auto-release.yml`'s `publish`/`release` jobs routinely lost that race, silently skipping the crates.io publish and/or GitHub Release step (reproduced on `v1.0.0`). The guard now keys off `inputs.tag == ''`, which reliably distinguishes the two paths regardless of what `github.event_name` reports.
