---
"moadim": patch
---

fix(ui,client): "Unassigned" Machine filter facet now matches blank machine entries

The Machine filter's "Unassigned" option checked `machines.is_empty()` (Rust UI) /
`machines.length > 0` (React client) against the raw machine array, so a legacy routine created
before the `validate_machines` guard (#600) — one still carrying a blank/whitespace-only entry
like `[""]` — would never match "Unassigned", even though the Dormant status facet and the
Machine filter dropdown (#1221, #1223) already treat that same shape as "no real machine
assigned". Both sides now check `machines.iter().all(|m| m.trim().is_empty())` /
`machines.every((m) => m.trim() === "")`, matching the established convention.
