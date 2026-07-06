---
"moadim": patch
---

### Fixed

Lower the background workbench-cleanup sweep from hourly to every 5 minutes, so a high-frequency routine (e.g. an every-minute schedule, whose effective TTL can be as low as ~60s) no longer piles up dozens of expired, finished workbenches — full repo clones included — between sweeps (#170). The max-runtime watchdog is unaffected; it already runs on its own 30s cadence.
