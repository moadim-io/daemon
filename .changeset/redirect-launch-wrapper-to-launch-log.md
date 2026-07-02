---
"moadim": patch
---

### Fixed

- **A failed routine launch left no trace anywhere.** The generated crontab line ran the prompt copy, the agent's `setup` step, and the `tmux` launch with no output redirection, so a failure in any of them (a `setup` error, `tmux new-session` failing, `PATH` not resolving `tmux`) went to cron's mail spool — silently discarded on the headless hosts this daemon targets, leaving no log to read next to the run's other artifacts. Everything after the workbench is created now runs inside a `{ … } >> "$WB/launch.log" 2>&1` group, so these failures are captured in the run's own workbench alongside `agent.log`. (#375)
