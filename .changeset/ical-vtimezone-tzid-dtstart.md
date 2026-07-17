---
"moadim": patch
---

fix(routines): TZID-qualify the `.ics` feed's `DTSTART` against an embedded `VTIMEZONE`

`build_ical` (`GET /routines.ics`) emitted every `VEVENT` as a bare UTC instant with no embedded
`VTIMEZONE`, per issue #387. The fire times themselves were correct (evaluated in the host's local
zone, matching crontab semantics), but with no timezone identity in the feed, a subscribing
calendar rendered each event in *its own* default zone rather than the host's — a routine scheduled
`0 9 * * *` on a `UTC+3` host displayed at 06:00 to a subscriber whose calendar defaults to UTC.

When the host's zone can be named (`iana_time_zone`/`local_timezone`), the feed now emits one
`VTIMEZONE` component (a `STANDARD` sub-component pinned to the feed's current UTC offset) and
qualifies each `DTSTART` as `DTSTART;TZID=<zone>:<local-wall-clock>`, so a subscriber sees the
routine's actual configured local time regardless of their calendar's own default zone. `DTSTAMP`
stays UTC as RFC 5545 requires. When the zone can't be named, the feed falls back to the original
bare UTC-instant `DTSTART` with no `VTIMEZONE`, exactly as before.

Scope: this does not model DST transition rules (a full `STANDARD`/`DAYLIGHT` pair with recurrence
rules would need a timezone-database dependency the daemon doesn't have). A routine in a
DST-observing zone may display shifted by the DST delta once the host crosses a transition after
the feed was generated — tracked as a follow-up on issue #387, which also covers the full
DST-aware acceptance criteria.
