---
"moadim": patch
---

feat(ui): add RefreshControl to heatmap page

The heatmap page previously used a hard-coded 30 s background refresh
with no user-visible indicator of when data was last loaded. It now
shows the same RefreshControl as the Overview and Routines pages
(Off / 5s / 15s / 30s / 60s dropdown + "updated N ago" freshness cue),
sharing the same localStorage key so the chosen cadence is consistent
across all pages.
