---
"moadim": patch
---

Move logging setup (`MOADIM_LOG_FORMAT`) into `src/logging/` module folder (`mod.rs` + `tests.rs`), matching the existing `src/utils/`/`src/paths/` convention. Pure file move, no behavior change (#852).
