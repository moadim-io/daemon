---
"moadim": patch
---

Enable `clippy::cast_possible_wrap`, the mirror image of `clippy::cast_sign_loss`, to reject an `as`-cast from an unsigned integer to a signed one, which silently wraps a value past the target's positive range into a negative one instead of erroring. Fixes the 2 violations this surfaced — `utils::time::format_local`'s Unix-seconds `u64` cast to the `i64` `chrono::Local::timestamp_opt` takes, and a test's `u32` child-process id cast to the `libc::pid_t` (`i32`) `libc::kill` takes — by converting to `<target>::try_from(...)`, clamped to the target's `MAX` on the theoretical overflow case instead of silently wrapping.
