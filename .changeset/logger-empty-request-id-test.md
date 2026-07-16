---
"moadim": patch
---

test(middlewares): cover the empty `x-request-id` header case in `logger`. The handler already falls back to a generated id when an inbound `x-request-id` is empty (`.filter(|header| !header.is_empty())`), but no test exercised that branch — a future edit removing the filter would silently start echoing back an empty correlation id. Test-only change, no behavior change.
