---
"moadim": patch
---

Fix `atomic_write` panicking the whole daemon on a write/sync I/O error (e.g. disk full) instead of returning it. `File::create`/`open` reserve no disk space, so `write_all`/`sync_all` can still fail after that call succeeds; they now propagate via `?` like every other step in `atomic_write`, instead of `.expect(...)`.
