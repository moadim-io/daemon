---
"moadim": patch
---

chore(lint): enable `clippy::ref_option` workspace-wide

Reject a `&Option<T>` parameter in favour of `Option<&T>` — the former forces every caller to
already own (or clone into) an `Option`, while the latter accepts a plain `&T` wrapped in `Some`
just as easily and is the idiomatic way to say "an optional borrow". Enabling it surfaced 1
violation in `ui/src/command_palette_match.rs`: `schedule_label` took `human: &Option<String>`
only to immediately match on it by reference. Changed the signature to `Option<&String>` and
updated its one call site and tests accordingly. No behavior change. The root `moadim` crate was
already clean, so `deny` there just locks it in.
