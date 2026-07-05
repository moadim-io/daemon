---
"moadim": minor
---

feat(ui): add TTL preset row (1h/1d/7d/30d) to the routine form

The WORKBENCH TTL input required typing a raw second count from memory,
unlike the SCHEDULE field which already has one-click cron presets. The
routine create/edit form now has a matching preset row under the TTL
input — 1h/1d/7d/30d buttons that set the field to the corresponding
second count — mirroring the cron schedule presets' styling and behavior.
