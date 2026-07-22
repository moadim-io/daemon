---
"moadim": patch
---

Rewrite enable_linger()'s if-let-Ok-unit pattern as .is_ok() (src/service/linux.rs). No behavior change -- clears the sole violation blocking clippy::equatable_if_let (PR #1391) from a clean CI run.
