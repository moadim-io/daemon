---
"moadim": patch
---

Fixed: deleting a routine while its agent was mid-run left that run executing unsupervised — the workbench's tmux session survived, untracked, until the next TTL sweep reaped the now-orphaned workbench (up to `effective_ttl_secs` later). `svc_delete` now force-kills any still-running session for the deleted routine's slug immediately (issue #333). The workbench directory itself is left in place and reaped normally.
