---
"moadim": patch
---

feat(ui): show routine health status tags in command palette subtitles

Routine entries in the ⌘K command palette previously showed only the
schedule description. They now suffix status tags so operators can see
health issues without leaving the palette:

- "DISABLED" — routine is turned off
- "SNOOZED" — skip_runs counter is active
- "AGENT MISSING" — agent not registered
- "FLAGS" — one or more open flags (appended alongside any other tag)

Six new host tests cover the combinations.
