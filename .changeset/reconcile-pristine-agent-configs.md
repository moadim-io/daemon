---
"moadim": patch
---

fix(routines): reconcile pristine built-in agent configs on startup

Built-in agent configs (`claude.toml`, `codex.toml`, `hermes.toml`) were only seeded when absent and never refreshed afterward — a shipped fix to a default agent config never reached an existing install. Startup now rewrites an existing config that is still pristine (unedited since the daemon wrote it) but stale, using a fingerprint header to distinguish pristine-but-stale from user-edited, mirroring the existing routine-defaults reconciliation. A user-edited config, or one with no managed header, is left untouched.
