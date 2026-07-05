---
"moadim": patch
---

feat(ui): show snoozed and flag indicators on day-timeline chips

Day-view timeline chips now carry two additional signals:

- **Snoozed routines** render at 45% opacity with an amber left-border
  instead of the standard accent border, so operators can distinguish
  suppressed fire times from active ones at a glance.
- **Flagged routines** show a red `⚑N` badge on the chip so pending
  flags are visible without leaving the timeline view.
