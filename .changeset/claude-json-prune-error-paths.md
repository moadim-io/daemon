---
"moadim": patch
---

Add tests exercising `prune_project_at`/`prune_locked`'s three previously branch-uncovered `?` error paths in `src/utils/claude_json.rs` (lock-file creation denied, `~/.claude.json` unreadable, and `atomic_write`'s temp-file creation denied). No behavior change — test-only.
