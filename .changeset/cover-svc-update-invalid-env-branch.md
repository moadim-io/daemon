---
"moadim": patch
---

test: cover `svc_update`'s invalid-env-key reject branch

`svc_update` calls the same `validate_env` used by `svc_create`, but only the `svc_create` side
had a test for the invalid-key rejection branch (`svc_create_rejects_invalid_env_key`). The
`svc_update` call at `validate_env(env)?` in `service_update.rs` was left untested, so
`cargo llvm-cov --fail-under-lines 100` (the repo's own CI and pre-push gate) fell short on that
line.

Adds `svc_update_rejects_invalid_env_key`, mirroring the existing `svc_update_rejects_blank_repository_url`
test shape, asserting the update is rejected with `BadRequest` and the routine's env map is left
untouched. No behavior change; test-only coverage fix.
