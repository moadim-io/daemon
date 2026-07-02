---
"moadim": patch
---

### Fixed

A routine whose composed prompt (prompt + repositories preamble + accumulated open flags) exceeded the OS per-argument limit (Linux `MAX_ARG_STRLEN`, 128 KiB) previously failed to launch with a silent, unreported `execve` error inside the detached tmux session — the run's health dot stayed green with no indication anything went wrong. This only affected agents (like the shipped `claude` default) whose config inlines the prompt via the `{prompt}` placeholder; `{prompt_file}`-based agents (`codex`, `hermes`) were never affected. The daemon now detects an oversized composed prompt before launching and skips the spawn with a visible warning instead.
