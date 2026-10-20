---
"moadim": minor
---

Add an optional per-routine `timezone` (IANA name) so a schedule can pin to a zone instead of always running in the host's own zone. The in-process scheduler fires the routine in that zone on every OS.
