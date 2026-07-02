---
"moadim": patch
---

Echo each request's log correlation id back as an `x-request-id` response header (`src/middlewares/logger.rs`), reusing an inbound `x-request-id` when the caller supplies one instead of always minting a fresh counter-based id. Completes the remaining acceptance criterion of issue #354; the shared inbound/outbound log correlation itself already shipped.
