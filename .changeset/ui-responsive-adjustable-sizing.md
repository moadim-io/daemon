---
"moadim": patch
---

### Changed

- Made the web UI's fixed-pixel dimensions fluid: command palette, modals,
  confirm dialog, filter input, and calendar/day nav widths now use
  `clamp()` instead of a single fixed width, and the schedule heatmap
  shrinks its cell/label sizing under 640px instead of relying only on
  horizontal scroll.
