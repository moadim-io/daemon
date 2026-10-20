---
"moadim": patch
---

Stop recreating a routine's `routine.toml` after it was deleted from disk while the daemon runs (for example by a git pull after the routine moved into a folder on another machine). Triggers, snoozes, flags, updates and the circuit breaker now refuse to write a routine whose file is gone instead of resurrecting a stale copy at its old path.
