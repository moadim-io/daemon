---
"moadim": patch
---

Fix `highlightSegments` (`client/src/pages/routines/logSearch.ts`) silently dropping a log-search
match when it starts on or spans a character whose `toLowerCase()` expands to more than one code
point (e.g. Turkish `İ` → `i` + a combining dot above). The per-character lowercase array lost its
1:1 correspondence with the original text in that case, misaligning every subsequent window in the
sliding-window match. Now truncates each mapped entry to its first code point, mirroring the Rust
port's `c.to_lowercase().next().unwrap_or(c)` in `ui/src/log_viewer.rs`. No behavior change for
plain-ASCII queries.
