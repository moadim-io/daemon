---
"moadim": patch
---

### Added

Regression test for `write_tmp`'s ENOSPC/EIO error path (via `/dev/full` on Linux), guarding the fix in #1019 where a full or failing disk during `atomic_write` now propagates the I/O error instead of panicking the whole daemon.
