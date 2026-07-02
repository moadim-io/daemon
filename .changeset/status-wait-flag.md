---
"moadim": minor
---

### Added

- **`moadim status --wait[=SECS]`.** Polls `GET /health` every 200ms until a
  server answers or `SECS` elapse (default 30) instead of checking once, so a
  launch script can block on startup (`moadim && moadim status --wait`) rather
  than sleeping a fixed guess before probing. Exits `0` once reachable and the
  existing `3` on timeout, matching the `status`/`cleanup`/`stop` exit-code
  contract.
