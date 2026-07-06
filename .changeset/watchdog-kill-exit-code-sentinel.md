---
"moadim": patch
---

Write a distinct `killed` sentinel to a watchdog-killed run's `exit_code` file so it never reads back as a misleading clean `0` exit (#453).
