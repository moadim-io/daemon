---
"moadim": patch
---

When a routine has several invalid `env` entries, the validation error now always reports the first invalid key in sorted order, instead of one that varied between runs.
