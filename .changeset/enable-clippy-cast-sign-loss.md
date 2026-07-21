---
"moadim": patch
---

Enable `clippy::cast_sign_loss` to reject an `as`-cast from a signed integer to an unsigned one, which silently wraps a negative value into a huge positive one instead of erroring. Fixes the 2 violations this surfaced — a Unix-timestamp `i64` cast to the `u64` seconds this crate stores timestamps as, in `logging::format_json_line` and `ical::feed` — by converting to `u64::try_from(...)` instead.
