---
"moadim": patch
---

feat(routines): show local human-readable time alongside raw timestamps

Run-history API responses (`RunSummary`/`FleetRunSummary`), the daemon's structured JSON log,
and the UI's relative-time displays now also expose an absolute, human-readable local-time form
next to the existing raw Unix timestamp / relative "N ago" text, so timestamps are readable
without doing epoch math.
