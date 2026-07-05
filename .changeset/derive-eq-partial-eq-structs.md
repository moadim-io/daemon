---
"moadim": patch
---

Derive `Eq` alongside `PartialEq` on `Flag`, `RunSummary`, and `FleetRunSummary` (all fields are already `Eq`-safe), and enable `clippy::derive_partial_eq_without_eq` to lock that in for future types.
