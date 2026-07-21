---
"moadim": patch
---

Enable `clippy::missing_const_for_fn` to catch functions whose bodies could run in `const` context but weren't marked `const fn`, closing off callers that need a `const`/`static` initializer or another `const fn` for no reason. Fixes the 7 violations this surfaced (`cli::liveness_exit_code`, `machine::MachineSource::label`, `routes::mcp::MoadimMcp::new`, `routines::cleanup::is_expired`, `routines::flags::FlagScope::suffix`, `routines::model::bool_true`, `routines::service_log_tail::LogWithMeta::empty`) by adding `const`. No behavior change.
