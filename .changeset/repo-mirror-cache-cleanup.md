---
"moadim": patch
---

fix(routines): prune the repository mirror cache under `{config_dir}/cache/` — orphaned mirrors (routine deleted or `repositories` URL edited) are now removed on every cleanup sweep, and an optional `MOADIM_MAX_REPO_CACHE_DISK_BYTES` ceiling evicts least-recently-fetched mirrors once the tree exceeds it, mirroring the existing `MOADIM_MAX_WORKBENCH_DISK_BYTES` safety valve. The tree's total size is now also reported via the new `moadim_repo_cache_bytes` metric (closes #1425).
