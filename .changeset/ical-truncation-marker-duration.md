---
"moadim": patch
---

fix(ical): give the schedule-truncation marker VEVENT a DURATION

The trailing "schedule truncated" marker event appended to the `.ics` feed
when a high-frequency routine hits the 100-event cap carried no `DURATION`
(unlike every regular fire event), so RFC 5545 treats it as a zero-length
instant. Most calendar UIs render a zero-length event as a barely-visible
sliver, defeating the marker's one job of telling subscribers the feed was
capped. It now carries the same `DURATION`/`TRANSP`/`X-MICROSOFT-CDO-BUSYSTATUS`
properties as a regular fire event.
