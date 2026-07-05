---
"moadim": minor
---

feat(ical): advertise REFRESH-INTERVAL & X-PUBLISHED-TTL on /routines.ics

The feed is regenerated on every request, but without a refresh hint
subscribers fall back to their own default polling interval (often 12-24h),
making routine schedule edits lag for hours before showing up in a
subscribed calendar. The feed now advertises both the RFC 7986 §5.7
`REFRESH-INTERVAL` property and the widely-honored Microsoft/Google
`X-PUBLISHED-TTL` fallback, both set to one hour.
