---
"moadim": patch
---

### Fixed

- **Routine VEVENTs in the `.ics` feed now carry a `DURATION`.** RFC 5545 requires a `VEVENT` to specify either `DTEND` or `DURATION`; without one, calendar clients rendered each fire as a zero-length instant. Every fire now emits `DURATION:PT15M`.
