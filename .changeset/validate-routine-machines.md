---
"moadim": patch
---

Reject empty/whitespace-only entries in a routine's `machines` targeting list on create and update, and trim + dedupe the accepted entries. Previously an unvalidated entry (e.g. `""` or `" host "`) could never match `machine::targets`' exact-string comparison, silently excluding the routine from every machine — and a list of only empty strings slipped past the dormant-routine warning entirely, since that check only fires on an empty list (#600).
