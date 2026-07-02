---
"moadim": patch
---

### Fixed

- **Workbench retention was measured from run trigger time, not finish time.** `effective_ttl_secs` is meant to keep a finished run around "only until the next run is due", but measuring age from the trigger timestamp subtracted the run's own duration from its retention window — a run whose duration exceeded its TTL was reaped on the very next sweep, sometimes seconds after completion. Retention is now based on when the run actually finished (`agent.log` mtime, clamped to at least the trigger time). (#174)
