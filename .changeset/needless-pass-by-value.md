---
"moadim": patch
---

Enable `clippy::needless_pass_by_value` and fix its three violations: `cli::trigger` now takes `&str` instead of an owned `String`, `cli_query::status_json` takes `Option<&HealthInfo>` instead of an owned `Option<HealthInfo>`, and `LockScope` derives `Copy` (a fieldless enum tag) instead of `global_lock::set_lock` taking it by value under the lint. No behavior change; avoids needless clones/moves at call sites.
