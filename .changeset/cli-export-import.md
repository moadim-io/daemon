---
"moadim": minor
---

Add offline `moadim export` / `moadim import` CLI commands to back up and restore the tracked config (routines, agents, `notifications.toml`, `user_prompt.md`) as a single portable JSON bundle, excluding gitignored runtime/local files. Import validates paths and `routine.toml` syntax before writing, writes atomically, skips existing files unless `--force`, and supports `--dry-run`.
