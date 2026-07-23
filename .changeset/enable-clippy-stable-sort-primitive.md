---
"moadim": patch
---

Enable clippy::stable_sort_primitive lint, rejecting stable sorts on primitive slices in favor of the faster unstable variant (#1396).
